//! Unified LSP sync manager with owned pending state.
//!
//! [`LspSyncManager`] owns all pending changes per document and tracks:
//! - Pending incremental change batches
//! - Full-sync escalation and initial open state
//! - Debounce scheduling and retry timing
//! - Single in-flight sends with write timeout
//!
//! # Error Handling
//!
//! - Queue full / server not ready: retryable, payload retained
//! - Other errors: escalate to full sync, set retry delay

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ropey::Rope;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use xeno_lsp::{DocumentSync, Error as LspError};
use xeno_primitives::LspDocumentChange;

use super::coalesce::coalesce_changes;
use crate::buffer::DocumentId;
use crate::metrics::EditorMetrics;

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

/// Timeout for waiting on write barrier completion before recovery.
pub const LSP_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// LSP document configuration.
#[derive(Debug, Clone)]
pub struct LspDocumentConfig {
	pub path: PathBuf,
	pub language: String,
	pub supports_incremental: bool,
}

/// Result of a flush attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushResult {
	/// Send succeeded.
	Success,
	/// Retryable error (backpressure, not ready). Don't escalate to full sync.
	Retryable,
	/// Non-recoverable error. Escalate to full sync.
	Failed,
}

impl FlushResult {
	/// Classify an LSP error into a flush result.
	fn from_error(err: &LspError) -> Self {
		match err {
			LspError::Backpressure | LspError::NotReady => FlushResult::Retryable,
			_ => FlushResult::Failed,
		}
	}
}

/// Completion message from spawned LSP send tasks.
#[derive(Debug)]
pub struct FlushComplete {
	pub doc_id: DocumentId,
	pub result: FlushResult,
	/// Whether this was a full sync (for expected_prev reset).
	pub was_full: bool,
}

/// Statistics from a flush operation.
#[derive(Debug, Default, Clone, Copy)]
pub struct FlushStats {
	pub flushed_docs: usize,
	pub full_syncs: u64,
	pub incremental_syncs: u64,
	pub snapshot_bytes: u64,
}

/// Current phase of a document's sync state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPhase {
	/// No pending changes or action needed.
	Idle,
	/// Changes pending, waiting for debounce period to elapse.
	Debouncing,
	/// A send is in progress, waiting for write barrier.
	InFlight,
}

/// Per-document sync state.
#[derive(Debug)]
pub struct DocSyncState {
	pub config: LspDocumentConfig,
	pub open_sent: bool,
	pub needs_full: bool,
	pub pending_changes: Vec<LspDocumentChange>,
	pub pending_bytes: usize,
	pub phase: SyncPhase,
	pub last_edit_at: Instant,
	pub retry_after: Option<Instant>,
	pub editor_version: u64,
	pub inflight: Option<InFlightInfo>,
	/// Expected previous version for contiguity checking.
	///
	/// Set after each successful commit to detect gaps or reorders.
	pub expected_prev: Option<u64>,
}

/// Metadata about an in-flight send.
#[derive(Debug, Clone)]
pub struct InFlightInfo {
	pub is_full: bool,
	pub version: u64,
	pub started_at: Instant,
}

impl DocSyncState {
	pub fn new(config: LspDocumentConfig, version: u64) -> Self {
		Self {
			config,
			open_sent: false,
			needs_full: true,
			pending_changes: Vec::new(),
			pending_bytes: 0,
			phase: SyncPhase::Idle,
			last_edit_at: Instant::now(),
			retry_after: None,
			editor_version: version,
			inflight: None,
			expected_prev: None,
		}
	}

	/// Records changes with contiguity checking.
	///
	/// Detects version gaps or reorders and escalates to full sync if detected.
	pub fn record_changes(
		&mut self,
		prev_version: u64,
		new_version: u64,
		changes: Vec<LspDocumentChange>,
		bytes: usize,
	) {
		self.last_edit_at = Instant::now();

		if let Some(expected) = self.expected_prev
			&& expected != prev_version
		{
			warn!(expected, got = prev_version, "lsp.sync.contiguity_break");
			self.escalate_full();
			self.expected_prev = Some(new_version);
			self.editor_version = new_version;
			if self.phase == SyncPhase::Idle {
				self.phase = SyncPhase::Debouncing;
			}
			return;
		}

		self.expected_prev = Some(new_version);
		self.editor_version = new_version;

		let new_count = self.pending_changes.len() + changes.len();
		let new_bytes = self.pending_bytes + bytes;

		if new_count > LSP_MAX_INCREMENTAL_CHANGES || new_bytes > LSP_MAX_INCREMENTAL_BYTES {
			self.escalate_full();
		} else {
			self.pending_changes.extend(changes);
			self.pending_bytes = new_bytes;
		}

		if self.phase == SyncPhase::Idle {
			self.phase = SyncPhase::Debouncing;
		}
	}

	pub fn escalate_full(&mut self) {
		self.needs_full = true;
		self.pending_changes.clear();
		self.pending_bytes = 0;
	}

	/// Full syncs bypass debounce; incremental syncs wait for it.
	pub fn is_due(&self, now: Instant, debounce: Duration) -> bool {
		if self.phase == SyncPhase::InFlight || self.retry_after.is_some_and(|t| now < t) {
			return false;
		}
		if self.needs_full {
			return true;
		}
		!self.pending_changes.is_empty() && now.duration_since(self.last_edit_at) >= debounce
	}

	pub fn mark_error_retry(&mut self) {
		self.retry_after = Some(Instant::now() + LSP_ERROR_RETRY_DELAY);
		self.phase = SyncPhase::Debouncing;
		self.inflight = None;
	}

	/// Takes pending payload and transitions to in-flight.
	pub fn take_for_send(&mut self, is_full: bool) -> (Vec<LspDocumentChange>, usize) {
		let changes = std::mem::take(&mut self.pending_changes);
		let bytes = self.pending_bytes;
		self.pending_bytes = 0;

		self.phase = SyncPhase::InFlight;
		self.inflight = Some(InFlightInfo {
			is_full,
			version: self.editor_version,
			started_at: Instant::now(),
		});

		if is_full {
			self.needs_full = false;
			self.open_sent = true;
		}

		(changes, bytes)
	}

	pub fn mark_complete(&mut self, result: FlushResult, was_full: bool) {
		self.inflight = None;

		match result {
			FlushResult::Success => {
				self.retry_after = None;
				self.phase = if self.pending_changes.is_empty() && !self.needs_full {
					SyncPhase::Idle
				} else {
					SyncPhase::Debouncing
				};
				if was_full {
					self.expected_prev = Some(self.editor_version);
				}
			}
			FlushResult::Retryable => self.mark_error_retry(),
			FlushResult::Failed => {
				self.mark_error_retry();
				self.needs_full = true;
			}
		}
	}

	/// Checks for write timeout, recovering with full sync escalation.
	pub fn check_write_timeout(&mut self, now: Instant, timeout: Duration) -> bool {
		let Some(ref info) = self.inflight else {
			return false;
		};
		if now.duration_since(info.started_at) <= timeout {
			return false;
		}

		warn!(
			version = info.version,
			is_full = info.is_full,
			"lsp.sync_write_timeout"
		);

		self.inflight = None;
		self.escalate_full();
		self.retry_after = Some(now + LSP_ERROR_RETRY_DELAY);
		self.phase = SyncPhase::Debouncing;
		true
	}
}

/// Unified LSP sync manager owning all pending changes per document.
pub struct LspSyncManager {
	docs: HashMap<DocumentId, DocSyncState>,
	completion_rx: mpsc::UnboundedReceiver<FlushComplete>,
	completion_tx: mpsc::UnboundedSender<FlushComplete>,
}

impl Default for LspSyncManager {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for LspSyncManager {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LspSyncManager")
			.field("docs", &self.docs.len())
			.finish()
	}
}

impl LspSyncManager {
	pub fn new() -> Self {
		let (completion_tx, completion_rx) = mpsc::unbounded_channel();
		Self {
			docs: HashMap::new(),
			completion_rx,
			completion_tx,
		}
	}

	pub fn on_doc_open(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
		debug!(doc_id = doc_id.0, path = ?config.path, version, "lsp.sync_manager.doc_open");
		self.docs.insert(doc_id, DocSyncState::new(config, version));
	}

	pub fn on_doc_close(&mut self, doc_id: DocumentId) {
		debug!(doc_id = doc_id.0, "lsp.sync_manager.doc_close");
		self.docs.remove(&doc_id);
	}

	/// Records edits for later sync. Untracked documents are silently ignored.
	pub fn on_doc_edit(
		&mut self,
		doc_id: DocumentId,
		prev_version: u64,
		new_version: u64,
		changes: Vec<LspDocumentChange>,
		bytes: usize,
	) {
		if let Some(state) = self.docs.get_mut(&doc_id) {
			tracing::trace!(
				doc_id = doc_id.0,
				prev_version,
				new_version,
				change_count = changes.len(),
				bytes,
				"lsp.sync_manager.doc_edit"
			);
			state.record_changes(prev_version, new_version, changes, bytes);
		}
	}

	pub fn escalate_full(&mut self, doc_id: DocumentId) {
		if let Some(state) = self.docs.get_mut(&doc_id) {
			debug!(doc_id = doc_id.0, "lsp.sync_manager.escalate_full");
			state.escalate_full();
		}
	}

	/// Takes pending changes for immediate sync, bypassing debounce.
	///
	/// Returns `(changes, needs_full, bytes)` or `None` if nothing pending.
	pub fn take_immediate(
		&mut self,
		doc_id: DocumentId,
	) -> Option<(Vec<LspDocumentChange>, bool, usize)> {
		let state = self.docs.get_mut(&doc_id)?;
		if state.pending_changes.is_empty() && !state.needs_full {
			return None;
		}

		let changes = std::mem::take(&mut state.pending_changes);
		let bytes = state.pending_bytes;
		state.pending_bytes = 0;
		let needs_full = state.needs_full;
		state.needs_full = false;
		state.phase = SyncPhase::Idle;

		Some((changes, needs_full, bytes))
	}

	#[cfg(test)]
	fn is_tracked(&self, doc_id: &DocumentId) -> bool {
		self.docs.contains_key(doc_id)
	}

	#[cfg(test)]
	fn doc_count(&self) -> usize {
		self.docs.len()
	}

	pub fn pending_count(&self) -> usize {
		self.docs
			.values()
			.filter(|s| s.phase == SyncPhase::Debouncing || !s.pending_changes.is_empty())
			.count()
	}

	pub fn in_flight_count(&self) -> usize {
		self.docs
			.values()
			.filter(|s| s.phase == SyncPhase::InFlight)
			.count()
	}

	fn poll_completions(&mut self) {
		while let Ok(complete) = self.completion_rx.try_recv() {
			if let Some(state) = self.docs.get_mut(&complete.doc_id) {
				state.mark_complete(complete.result, complete.was_full);
			}
		}
	}

	/// Flushes documents that are due for sync.
	///
	/// When `client_ready` is `false`, skips flushing but still polls
	/// completions and checks for write timeouts.
	pub fn tick<F>(
		&mut self,
		now: Instant,
		client_ready: bool,
		sync: &DocumentSync,
		metrics: &Arc<EditorMetrics>,
		snapshot_provider: F,
	) -> FlushStats
	where
		F: Fn(DocumentId) -> Option<(Rope, u64)>,
	{
		self.poll_completions();

		for state in self.docs.values_mut() {
			state.check_write_timeout(now, LSP_WRITE_TIMEOUT);
		}

		if !client_ready {
			return FlushStats::default();
		}

		let mut stats = FlushStats::default();

		let due_docs: Vec<_> = self
			.docs
			.iter()
			.filter(|(_, state)| state.is_due(now, LSP_DEBOUNCE))
			.map(|(&doc_id, _)| doc_id)
			.take(LSP_MAX_DOCS_PER_TICK)
			.collect();

		for doc_id in due_docs {
			if stats.flushed_docs >= LSP_MAX_DOCS_PER_TICK {
				break;
			}

			let Some(state) = self.docs.get_mut(&doc_id) else {
				continue;
			};

			let path = state.config.path.clone();
			let language = state.config.language.clone();
			let use_full = state.needs_full || !state.config.supports_incremental;
			let editor_version = state.editor_version;

			if use_full {
				let Some((content, snapshot_version)) = snapshot_provider(doc_id) else {
					warn!(doc_id = doc_id.0, "lsp.sync_manager.no_snapshot");
					continue;
				};

				let _ = state.take_for_send(true);
				let snapshot_bytes = content.len_bytes() as u64;
				stats.full_syncs += 1;
				stats.snapshot_bytes += snapshot_bytes;

				debug!(
					doc_id = doc_id.0,
					path = ?path,
					mode = "full",
					snapshot_version,
					editor_version,
					"lsp.sync_manager.flush_start"
				);

				let sync = sync.clone();
				let tx = self.completion_tx.clone();
				let metrics = metrics.clone();

				tokio::spawn(async move {
					let start = Instant::now();
					let snapshot = content.to_string();
					metrics.add_snapshot_bytes(snapshot_bytes);

					let result = sync
						.notify_change_full_with_barrier_text(&path, &language, snapshot)
						.await;
					let latency_ms = start.elapsed().as_millis() as u64;

					let flush_result = match &result {
						Ok(_) => {
							metrics.inc_full_sync();
							debug!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, "lsp.sync_manager.flush_done");
							FlushResult::Success
						}
						Err(err) => {
							metrics.inc_send_error();
							let classified = FlushResult::from_error(err);
							if classified == FlushResult::Retryable {
								debug!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, error = ?err, "lsp.sync_manager.flush_retryable");
							} else {
								warn!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, error = ?err, "lsp.sync_manager.flush_failed");
							}
							classified
						}
					};

					let _ = tx.send(FlushComplete {
						doc_id,
						result: flush_result,
						was_full: true,
					});
				});
			} else {
				let (raw_changes, _) = state.take_for_send(false);
				let raw_count = raw_changes.len();
				let changes = coalesce_changes(raw_changes);
				let coalesced = raw_count.saturating_sub(changes.len());

				if coalesced > 0 {
					metrics.add_coalesced(coalesced as u64);
				}

				stats.incremental_syncs += 1;

				debug!(
					doc_id = doc_id.0,
					path = ?path,
					mode = "incremental",
					raw_count,
					change_count = changes.len(),
					coalesced,
					editor_version,
					"lsp.sync_manager.flush_start"
				);

				let sync = sync.clone();
				let tx = self.completion_tx.clone();
				let metrics = metrics.clone();

				tokio::spawn(async move {
					let start = Instant::now();
					let result = sync
						.notify_change_incremental_no_content_with_barrier(
							&path, &language, changes,
						)
						.await;
					let latency_ms = start.elapsed().as_millis() as u64;

					let flush_result = match &result {
						Ok(_) => {
							metrics.inc_incremental_sync();
							debug!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, "lsp.sync_manager.flush_done");
							FlushResult::Success
						}
						Err(err) => {
							metrics.inc_send_error();
							let classified = FlushResult::from_error(err);
							if classified == FlushResult::Retryable {
								debug!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, error = ?err, "lsp.sync_manager.flush_retryable");
							} else {
								warn!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, error = ?err, "lsp.sync_manager.flush_failed");
							}
							classified
						}
					};

					let _ = tx.send(FlushComplete {
						doc_id,
						result: flush_result,
						was_full: false,
					});
				});
			}

			if let Some(state) = self.docs.get_mut(&doc_id) {
				state.retry_after = None;
			}

			stats.flushed_docs += 1;
		}

		stats
	}
}

#[cfg(test)]
mod tests;
