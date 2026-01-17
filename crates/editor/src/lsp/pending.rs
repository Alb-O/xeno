//! Pending LSP change accumulator for debounced sync.
//!
//! This module provides [`PendingLspState`] for accumulating document changes
//! across ticks, enabling debounced LSP notifications instead of per-tick sends.
//!
//! # Design
//!
//! Instead of sending an LSP `didChange` notification on every tick, changes
//! are accumulated in a per-document [`PendingLsp`] struct. The main loop
//! calls [`flush_due`] with the current time to send notifications for
//! documents whose debounce period has elapsed.
//!
//! # Thresholds
//!
//! When accumulated changes exceed configured thresholds (count or bytes),
//! the document is marked for full sync. This prevents unbounded memory
//! growth and handles cases where incremental sync would be inefficient.
//!
//! # Single-Flight Sends
//!
//! Each document can have at most one in-flight send at a time. If new edits
//! arrive while a send is in progress, they accumulate in pending state and
//! are sent after the current send completes.
//!
//! # Error Recovery
//!
//! On send failure, the document is marked for full sync with a retry delay.
//! This ensures the system converges back to a correct state.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ropey::Rope;
use tokio::sync::mpsc;
use tracing::debug;
use xeno_lsp::{DocumentSync, OffsetEncoding};
use xeno_primitives::LspDocumentChange;

use crate::buffer::DocumentId;

/// Default debounce duration for LSP notifications.
pub const LSP_DEBOUNCE: Duration = Duration::from_millis(30);

/// Maximum number of incremental changes before falling back to full sync.
pub const LSP_MAX_INCREMENTAL_CHANGES: usize = 100;

/// Maximum total bytes of inserted text before falling back to full sync.
pub const LSP_MAX_INCREMENTAL_BYTES: usize = 100 * 1024; // 100 KB

/// Retry delay after LSP send failure before attempting full sync.
pub const LSP_ERROR_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Pending LSP changes for a single document.
#[derive(Debug)]
pub struct PendingLsp {
	/// When the most recent edit occurred.
	pub last_edit_at: Instant,
	/// Force a full sync (threshold exceeded or fallback required).
	pub force_full: bool,
	/// Accumulated changes since last flush.
	pub changes: Vec<LspDocumentChange>,
	/// Total bytes in accumulated change text.
	pub bytes: usize,
	/// File path for this document.
	pub path: PathBuf,
	/// Language ID for this document.
	pub language: String,
	/// Whether this document supports incremental sync.
	pub supports_incremental: bool,
	/// Offset encoding for the LSP server.
	pub encoding: OffsetEncoding,
	/// Editor document version when changes were accumulated.
	pub editor_version: u64,
	/// Earliest time a retry is allowed (after error).
	pub retry_after: Option<Instant>,
}

impl PendingLsp {
	/// Creates a new pending state for a document.
	pub fn new(
		path: PathBuf,
		language: String,
		supports_incremental: bool,
		encoding: OffsetEncoding,
		editor_version: u64,
	) -> Self {
		Self {
			last_edit_at: Instant::now(),
			force_full: false,
			changes: Vec::new(),
			bytes: 0,
			path,
			language,
			supports_incremental,
			encoding,
			editor_version,
			retry_after: None,
		}
	}

	/// Appends changes to the pending queue.
	///
	/// If thresholds are exceeded, marks for full sync.
	pub fn append_changes(
		&mut self,
		new_changes: Vec<LspDocumentChange>,
		force_full: bool,
		editor_version: u64,
	) {
		self.last_edit_at = Instant::now();
		self.editor_version = editor_version;

		if force_full {
			self.force_full = true;
			self.changes.clear();
			self.bytes = 0;
			return;
		}

		if self.force_full {
			return;
		}

		let new_bytes: usize = new_changes.iter().map(|c| c.new_text.len()).sum();
		self.bytes += new_bytes;
		self.changes.extend(new_changes);

		if self.changes.len() > LSP_MAX_INCREMENTAL_CHANGES
			|| self.bytes > LSP_MAX_INCREMENTAL_BYTES
		{
			self.force_full = true;
			self.changes.clear();
			self.bytes = 0;
		}
	}

	/// Returns true if this pending state should be flushed.
	pub fn is_due(&self, now: Instant, debounce: Duration) -> bool {
		if self.retry_after.is_some_and(|t| now < t) {
			return false;
		}
		now.duration_since(self.last_edit_at) >= debounce || self.force_full
	}

	/// Returns true if incremental sync should be used.
	pub fn use_incremental(&self) -> bool {
		!self.force_full && self.supports_incremental && !self.changes.is_empty()
	}

	/// Marks this pending state for full sync retry after an error.
	pub fn mark_error_retry(&mut self) {
		self.force_full = true;
		self.changes.clear();
		self.bytes = 0;
		self.retry_after = Some(Instant::now() + LSP_ERROR_RETRY_DELAY);
	}
}

/// Message sent back from spawned LSP tasks to report completion.
#[derive(Debug)]
pub struct FlushComplete {
	/// Document that completed.
	pub doc_id: DocumentId,
	/// Whether the send succeeded.
	pub success: bool,
}

/// Accumulated pending LSP state across all documents.
pub struct PendingLspState {
	/// Per-document pending changes.
	pending: HashMap<DocumentId, PendingLsp>,
	/// Documents with in-flight sends.
	in_flight: HashSet<DocumentId>,
	/// Channel for receiving flush completions.
	completion_rx: mpsc::UnboundedReceiver<FlushComplete>,
	/// Sender cloned for spawned tasks.
	completion_tx: mpsc::UnboundedSender<FlushComplete>,
}

impl Default for PendingLspState {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for PendingLspState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PendingLspState")
			.field("pending", &self.pending)
			.field("in_flight", &self.in_flight)
			.finish()
	}
}

impl PendingLspState {
	/// Creates a new empty pending state.
	pub fn new() -> Self {
		let (completion_tx, completion_rx) = mpsc::unbounded_channel();
		Self {
			pending: HashMap::new(),
			in_flight: HashSet::new(),
			completion_rx,
			completion_tx,
		}
	}

	/// Polls for completed flushes and updates in-flight state.
	pub fn poll_completions(&mut self) {
		while let Ok(complete) = self.completion_rx.try_recv() {
			self.in_flight.remove(&complete.doc_id);
			if !complete.success {
				if let Some(pending) = self.pending.get_mut(&complete.doc_id) {
					pending.mark_error_retry();
				}
			}
		}
	}

	/// Accumulates changes for a document.
	///
	/// Call this instead of immediately spawning an LSP notification task.
	pub fn accumulate(
		&mut self,
		doc_id: DocumentId,
		path: PathBuf,
		language: String,
		changes: Vec<LspDocumentChange>,
		force_full: bool,
		supports_incremental: bool,
		encoding: OffsetEncoding,
		editor_version: u64,
	) {
		let entry = self.pending.entry(doc_id).or_insert_with(|| {
			PendingLsp::new(
				path.clone(),
				language.clone(),
				supports_incremental,
				encoding,
				editor_version,
			)
		});

		entry.path = path;
		entry.language = language;
		entry.supports_incremental = supports_incremental;
		entry.encoding = encoding;
		entry.append_changes(changes, force_full, editor_version);
	}

	/// Flushes due documents and spawns LSP notification tasks.
	pub fn flush_due(
		&mut self,
		now: Instant,
		debounce: Duration,
		sync: &DocumentSync,
		content_provider: impl Fn(DocumentId) -> Option<Rope>,
	) -> usize {
		self.poll_completions();

		let mut flushed = 0;
		let mut to_remove = Vec::new();

		for (&doc_id, pending) in &mut self.pending {
			if self.in_flight.contains(&doc_id) || !pending.is_due(now, debounce) {
				continue;
			}

			let path = pending.path.clone();
			let language = pending.language.clone();
			let use_incremental = pending.use_incremental();
			let changes = std::mem::take(&mut pending.changes);

			debug!(
				path = ?path,
				mode = if use_incremental { "incremental" } else { "full" },
				change_count = changes.len(),
				bytes = pending.bytes,
				editor_version = pending.editor_version,
				"LSP flush triggered"
			);

			let sync = sync.clone();
			let tx = self.completion_tx.clone();
			self.in_flight.insert(doc_id);

			if use_incremental {
				tokio::spawn(async move {
					let success =
						sync.notify_change_incremental_no_content(&path, &language, changes)
							.await
							.is_ok();
					if !success {
						tracing::warn!(path = ?path, "LSP incremental change failed, will retry with full sync");
					}
					let _ = tx.send(FlushComplete { doc_id, success });
				});
			} else {
				let Some(content) = content_provider(doc_id) else {
					self.in_flight.remove(&doc_id);
					continue;
				};
				tokio::spawn(async move {
					let success = sync
						.notify_change_full(&path, &language, &content)
						.await
						.is_ok();
					if !success {
						tracing::warn!(path = ?path, "LSP full change failed, will retry");
					}
					let _ = tx.send(FlushComplete { doc_id, success });
				});
			}

			pending.bytes = 0;
			pending.force_full = false;
			pending.retry_after = None;
			to_remove.push(doc_id);
			flushed += 1;
		}

		for doc_id in to_remove {
			self.pending.remove(&doc_id);
		}

		flushed
	}

	/// Returns the number of documents with pending changes.
	pub fn pending_count(&self) -> usize {
		self.pending.len()
	}

	/// Returns the number of documents with in-flight sends.
	pub fn in_flight_count(&self) -> usize {
		self.in_flight.len()
	}

	/// Returns true if there are any pending changes.
	pub fn has_pending(&self) -> bool {
		!self.pending.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_pending_lsp_append_respects_thresholds() {
		let mut pending = PendingLsp::new(
			PathBuf::from("test.rs"),
			"rust".to_string(),
			true,
			OffsetEncoding::Utf16,
			0,
		);

		// Append many small changes
		for i in 0..LSP_MAX_INCREMENTAL_CHANGES + 1 {
			pending.append_changes(
				vec![LspDocumentChange {
					range: xeno_primitives::lsp::LspRange::point(
						xeno_primitives::lsp::LspPosition::new(0, 0),
					),
					new_text: "x".to_string(),
				}],
				false,
				i as u64,
			);
		}

		assert!(pending.force_full);
		assert!(pending.changes.is_empty());
	}

	#[test]
	fn test_pending_lsp_is_due_respects_debounce() {
		let pending = PendingLsp::new(
			PathBuf::from("test.rs"),
			"rust".to_string(),
			true,
			OffsetEncoding::Utf16,
			0,
		);

		// Just created, not due yet
		assert!(!pending.is_due(Instant::now(), LSP_DEBOUNCE));
	}

	#[test]
	fn test_pending_lsp_force_full_is_due_immediately() {
		let mut pending = PendingLsp::new(
			PathBuf::from("test.rs"),
			"rust".to_string(),
			true,
			OffsetEncoding::Utf16,
			0,
		);
		pending.force_full = true;

		assert!(pending.is_due(Instant::now(), LSP_DEBOUNCE));
	}

	#[test]
	fn test_pending_lsp_retry_after_delays_flush() {
		let mut pending = PendingLsp::new(
			PathBuf::from("test.rs"),
			"rust".to_string(),
			true,
			OffsetEncoding::Utf16,
			0,
		);
		pending.force_full = true;
		pending.retry_after = Some(Instant::now() + Duration::from_secs(1));

		assert!(!pending.is_due(Instant::now(), LSP_DEBOUNCE));
	}

	#[test]
	fn test_pending_state_accumulate_updates_version() {
		let mut state = PendingLspState::new();

		state.accumulate(
			DocumentId(1),
			PathBuf::from("test.rs"),
			"rust".to_string(),
			vec![],
			false,
			true,
			OffsetEncoding::Utf16,
			42,
		);

		assert_eq!(state.pending.get(&DocumentId(1)).unwrap().editor_version, 42);
	}
}
