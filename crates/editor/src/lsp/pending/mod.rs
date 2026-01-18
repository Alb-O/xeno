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
use std::sync::Arc;
use std::time::{Duration, Instant};

use ropey::Rope;
use tokio::sync::mpsc;
use tracing::debug;
use xeno_lsp::{DocumentSync, OffsetEncoding};
use xeno_primitives::LspDocumentChange;

use super::coalesce::coalesce_changes;
use crate::buffer::DocumentId;
use crate::metrics::EditorMetrics;

#[cfg(test)]
mod tests;

/// Default debounce duration for LSP notifications.
pub const LSP_DEBOUNCE: Duration = Duration::from_millis(30);

/// Maximum number of incremental changes before falling back to full sync.
pub const LSP_MAX_INCREMENTAL_CHANGES: usize = 100;

/// Maximum total bytes of inserted text before falling back to full sync (100 KB).
pub const LSP_MAX_INCREMENTAL_BYTES: usize = 100 * 1024;

/// Retry delay after LSP send failure before attempting full sync.
pub const LSP_ERROR_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Maximum number of documents flushed per tick.
pub const LSP_MAX_DOCS_PER_TICK: usize = 8;

/// LSP document configuration for change accumulation.
#[derive(Debug, Clone)]
pub struct LspDocumentConfig {
	/// File path for this document.
	pub path: PathBuf,
	/// Language ID for this document.
	pub language: String,
	/// Whether this document supports incremental sync.
	pub supports_incremental: bool,
	/// Offset encoding for the LSP server.
	pub encoding: OffsetEncoding,
}

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
	pub fn new(config: LspDocumentConfig, editor_version: u64) -> Self {
		Self {
			last_edit_at: Instant::now(),
			force_full: false,
			changes: Vec::new(),
			bytes: 0,
			path: config.path,
			language: config.language,
			supports_incremental: config.supports_incremental,
			encoding: config.encoding,
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

	/// Updates the document configuration fields.
	fn update_config(&mut self, config: LspDocumentConfig) {
		self.path = config.path;
		self.language = config.language;
		self.supports_incremental = config.supports_incremental;
		self.encoding = config.encoding;
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

#[derive(Debug, Default, Clone, Copy)]
pub struct FlushStats {
	pub flushed_docs: usize,
	pub full_syncs: u64,
	pub incremental_syncs: u64,
	pub snapshot_bytes: u64,
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
			if !complete.success
				&& let Some(pending) = self.pending.get_mut(&complete.doc_id)
			{
				pending.mark_error_retry();
			}
		}
	}

	/// Returns whether a document has an in-flight send.
	pub fn is_in_flight(&self, doc_id: &DocumentId) -> bool {
		self.in_flight.contains(doc_id)
	}

	/// Marks a document as having an in-flight send.
	#[cfg(test)]
	pub fn mark_in_flight(&mut self, doc_id: DocumentId) {
		self.in_flight.insert(doc_id);
	}

	/// Clears the in-flight state for a document.
	#[cfg(test)]
	pub fn clear_in_flight(&mut self, doc_id: &DocumentId) {
		self.in_flight.remove(doc_id);
	}

	/// Accumulates changes for a document.
	///
	/// Call this instead of immediately spawning an LSP notification task.
	pub fn accumulate(
		&mut self,
		doc_id: DocumentId,
		config: LspDocumentConfig,
		changes: Vec<LspDocumentChange>,
		force_full: bool,
		editor_version: u64,
	) {
		let added_changes = changes.len();
		let added_bytes: usize = changes.iter().map(|c| c.new_text.len()).sum();

		let entry = self
			.pending
			.entry(doc_id)
			.or_insert_with(|| PendingLsp::new(config.clone(), editor_version));

		entry.update_config(config);
		entry.append_changes(changes, force_full, editor_version);

		tracing::trace!(
			doc_id = doc_id.0,
			added_changes,
			added_bytes,
			total_changes = entry.changes.len(),
			total_bytes = entry.bytes,
			force_full = entry.force_full,
			"lsp.pending_append"
		);
	}

	/// Flushes due documents and spawns LSP notification tasks.
	pub fn flush_due(
		&mut self,
		now: Instant,
		debounce: Duration,
		max_docs: usize,
		sync: &DocumentSync,
		metrics: &Arc<EditorMetrics>,
		content_provider: impl Fn(DocumentId) -> Option<Rope>,
	) -> FlushStats {
		self.poll_completions();

		let mut stats = FlushStats::default();
		let mut to_remove = Vec::new();

		if max_docs == 0 {
			return stats;
		}

		for (&doc_id, pending) in &mut self.pending {
			if stats.flushed_docs >= max_docs {
				break;
			}

			if self.in_flight.contains(&doc_id) || !pending.is_due(now, debounce) {
				continue;
			}

			let path = pending.path.clone();
			let language = pending.language.clone();
			let use_incremental = pending.use_incremental();
			let raw_changes = std::mem::take(&mut pending.changes);
			let raw_count = raw_changes.len();
			let changes = coalesce_changes(raw_changes);
			let change_count = changes.len();
			let bytes = pending.bytes;
			let editor_version = pending.editor_version;
			let mode = if use_incremental {
				"incremental"
			} else {
				"full"
			};

			debug!(
				doc_id = doc_id.0,
				path = ?path,
				mode,
				raw_count,
				change_count,
				coalesced = raw_count.saturating_sub(change_count),
				bytes,
				editor_version,
				"lsp.flush_start"
			);

			let coalesced = raw_count.saturating_sub(change_count);
			if coalesced > 0 {
				metrics.add_coalesced(coalesced as u64);
			}

			let sync = sync.clone();
			let tx = self.completion_tx.clone();
			let metrics = metrics.clone();
			self.in_flight.insert(doc_id);

			if use_incremental {
				stats.incremental_syncs += 1;
				tokio::spawn(async move {
					let start = Instant::now();
					let result = sync
						.notify_change_incremental_no_content_with_ack(&path, &language, changes)
						.await;
					let success = result.is_ok();
					let latency_ms = start.elapsed().as_millis() as u64;

					if success {
						metrics.inc_incremental_sync();
						tracing::debug!(
							doc_id = doc_id.0,
							path = ?path,
							mode = "incremental",
							latency_ms,
							editor_version,
							"lsp.flush_done"
						);
					} else {
						metrics.inc_send_error();
						tracing::warn!(
							doc_id = doc_id.0,
							path = ?path,
							mode = "incremental",
							latency_ms,
							error = ?result.err(),
							"lsp.flush_done"
						);
					}
					let _ = tx.send(FlushComplete { doc_id, success });
				});
			} else {
				let Some(content) = content_provider(doc_id) else {
					self.in_flight.remove(&doc_id);
					continue;
				};
				let snapshot_bytes = content.len_bytes() as u64;
				stats.full_syncs += 1;
				stats.snapshot_bytes += snapshot_bytes;
				tokio::spawn(async move {
					let start = Instant::now();
					let snapshot = content.to_string();
					metrics.add_snapshot_bytes(snapshot_bytes);
					let result = sync
						.notify_change_full_with_ack_text(&path, &language, snapshot)
						.await;
					let success = result.is_ok();
					let latency_ms = start.elapsed().as_millis() as u64;

					if success {
						metrics.inc_full_sync();
						tracing::debug!(
							doc_id = doc_id.0,
							path = ?path,
							mode = "full",
							latency_ms,
							editor_version,
							"lsp.flush_done"
						);
					} else {
						metrics.inc_send_error();
						tracing::warn!(
							doc_id = doc_id.0,
							path = ?path,
							mode = "full",
							latency_ms,
							error = ?result.err(),
							"lsp.flush_done"
						);
					}
					let _ = tx.send(FlushComplete { doc_id, success });
				});
			}

			pending.bytes = 0;
			pending.force_full = false;
			pending.retry_after = None;
			to_remove.push(doc_id);
			stats.flushed_docs += 1;
		}

		for doc_id in to_remove {
			self.pending.remove(&doc_id);
		}

		stats
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
