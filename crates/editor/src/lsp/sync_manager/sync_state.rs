use std::path::PathBuf;
use std::time::{Duration, Instant};

use tracing::warn;
use xeno_lsp::Error as LspError;
use xeno_primitives::LspDocumentChange;

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
	pub(super) fn from_error(err: &LspError) -> Self {
		match err {
			LspError::Backpressure | LspError::NotReady => FlushResult::Retryable,
			_ => FlushResult::Failed,
		}
	}
}

/// Completion message from spawned LSP send tasks.
#[derive(Debug)]
pub struct FlushComplete {
	pub doc_id: crate::core::document::DocumentId,
	pub generation: u64,
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

/// Metadata about an in-flight send.
#[derive(Debug, Clone)]
pub struct InFlightInfo {
	pub is_full: bool,
	pub version: u64,
	pub started_at: Instant,
}

/// Per-document sync state.
#[derive(Debug)]
pub struct DocSyncState {
	pub generation: u64,
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

impl DocSyncState {
	pub fn new(config: LspDocumentConfig, version: u64) -> Self {
		Self {
			generation: 0,
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
	pub fn record_changes(&mut self, prev_version: u64, new_version: u64, changes: Vec<LspDocumentChange>, bytes: usize) {
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
			FlushResult::Retryable => {
				self.mark_error_retry();
			}
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

		warn!(version = info.version, is_full = info.is_full, "lsp.sync_write_timeout");

		self.inflight = None;
		self.escalate_full();
		self.retry_after = Some(now + LSP_ERROR_RETRY_DELAY);
		self.phase = SyncPhase::Debouncing;
		true
	}
}
