//! Document state for LSP tracking.
//!
//! Tracks version numbers, diagnostics, and other LSP-related metadata
//! for individual documents.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use lsp_types::{Diagnostic, DiagnosticSeverity, Uri};
use parking_lot::RwLock;

#[derive(Debug, Default)]
struct SyncState {
	pending_versions: VecDeque<i32>,
	acked_version: i32,
	force_full_sync: bool,
}

/// LSP state for a single document.
///
/// Tracks version number for incremental sync, diagnostics, and other
/// LSP-related metadata.
#[derive(Debug)]
pub struct DocumentState {
	/// Document URI (derived from file path).
	uri: Uri,
	/// Document version for LSP sync. Incremented on each change.
	version: AtomicI32,
	/// Whether the document has been opened with the language server.
	opened: RwLock<bool>,
	/// Current diagnostics from the language server.
	diagnostics: RwLock<Vec<Diagnostic>>,
	/// Language ID for the document (e.g., "rust", "python").
	language_id: RwLock<Option<String>>,
	/// Sync state for tracking pending sends and mismatches.
	sync_state: RwLock<SyncState>,
	/// Session generation, assigned by [`DocumentStateManager`] on each
	/// [`mark_opened`](Self::mark_opened) call. Barriers capture this value
	/// at creation time and validate it on completion so that stale barriers
	/// from a previous open session are silently ignored.
	generation: AtomicU64,
}

impl DocumentState {
	/// Create a new document state from a file path.
	///
	/// Returns `None` if the path cannot be converted to a URL.
	pub fn new(path: &Path) -> Option<Self> {
		let uri = crate::uri_from_path(path)?;
		Some(Self {
			uri,
			version: AtomicI32::new(0),
			opened: RwLock::new(false),
			diagnostics: RwLock::new(Vec::new()),
			language_id: RwLock::new(None),
			sync_state: RwLock::new(SyncState::default()),
			generation: AtomicU64::new(0),
		})
	}

	/// Create a document state from a URI directly.
	pub fn from_uri(uri: Uri) -> Self {
		Self {
			uri,
			version: AtomicI32::new(0),
			opened: RwLock::new(false),
			diagnostics: RwLock::new(Vec::new()),
			language_id: RwLock::new(None),
			sync_state: RwLock::new(SyncState::default()),
			generation: AtomicU64::new(0),
		}
	}

	/// Get the document URI.
	pub fn uri(&self) -> &Uri {
		&self.uri
	}

	/// Get the current document version.
	pub fn version(&self) -> i32 {
		self.version.load(Ordering::Relaxed)
	}

	/// Increment the version and return the new value.
	///
	/// Should be called whenever the document content changes.
	pub fn increment_version(&self) -> i32 {
		self.version.fetch_add(1, Ordering::Relaxed) + 1
	}

	/// Check if the document has been opened with a language server.
	pub fn is_opened(&self) -> bool {
		*self.opened.read()
	}

	/// Mark the document as opened with a language server.
	pub fn set_opened(&self, opened: bool) {
		*self.opened.write() = opened;
	}

	/// Marks the document as opened and resets sync state.
	///
	/// `generation` is a globally unique session ID assigned by the
	/// [`DocumentStateManager`] so that barriers from a previous open
	/// session can be detected and ignored on completion.
	pub fn mark_opened(&self, version: i32, generation: u64) {
		*self.opened.write() = true;
		self.generation.store(generation, Ordering::Relaxed);
		let mut sync = self.sync_state.write();
		sync.pending_versions.clear();
		sync.acked_version = version;
		sync.force_full_sync = false;
	}

	/// Returns the current session generation.
	pub fn generation(&self) -> u64 {
		self.generation.load(Ordering::Relaxed)
	}

	/// Get the language ID.
	pub fn language_id(&self) -> Option<String> {
		self.language_id.read().clone()
	}

	/// Set the language ID.
	pub fn set_language_id(&self, lang: impl Into<String>) {
		*self.language_id.write() = Some(lang.into());
	}

	/// Returns the current acked version (best-effort).
	pub fn acked_version(&self) -> i32 {
		self.sync_state.read().acked_version
	}

	/// Returns the number of pending change versions.
	pub fn pending_versions_len(&self) -> usize {
		self.sync_state.read().pending_versions.len()
	}

	/// Increments the version and records it as pending.
	pub fn next_version(&self) -> i32 {
		let version = self.increment_version();
		self.sync_state.write().pending_versions.push_back(version);
		version
	}

	/// Acknowledge a pending version in FIFO order.
	pub fn ack_version(&self, version: i32) -> bool {
		let mut sync = self.sync_state.write();
		let Some(expected) = sync.pending_versions.pop_front() else {
			sync.force_full_sync = true;
			return false;
		};

		if expected != version {
			sync.pending_versions.clear();
			sync.force_full_sync = true;
			return false;
		}

		sync.acked_version = version;
		true
	}

	/// Marks this document as requiring a full sync.
	pub fn mark_force_full_sync(&self) {
		let mut sync = self.sync_state.write();
		sync.force_full_sync = true;
		sync.pending_versions.clear();
	}

	/// Takes and clears the force-full-sync flag.
	pub fn take_force_full_sync(&self) -> bool {
		let mut sync = self.sync_state.write();
		let needs = sync.force_full_sync;
		sync.force_full_sync = false;
		needs
	}

	/// Records a diagnostics version and marks for full sync on mismatch.
	pub fn record_diagnostics_version(&self, version: i32) -> bool {
		let mut sync = self.sync_state.write();
		if !sync.pending_versions.is_empty() {
			return false;
		}

		let last_sent = self.version();
		if version < last_sent {
			sync.force_full_sync = true;
			return true;
		}

		false
	}

	/// Get all diagnostics for this document.
	pub fn diagnostics(&self) -> Vec<Diagnostic> {
		self.diagnostics.read().clone()
	}

	/// Set diagnostics for this document.
	pub fn set_diagnostics(&self, diagnostics: Vec<Diagnostic>) {
		*self.diagnostics.write() = diagnostics;
	}

	/// Clear all diagnostics.
	pub fn clear_diagnostics(&self) {
		self.diagnostics.write().clear();
	}

	/// Get diagnostics filtered by severity.
	pub fn diagnostics_by_severity(&self, severity: DiagnosticSeverity) -> Vec<Diagnostic> {
		self.diagnostics.read().iter().filter(|d| d.severity == Some(severity)).cloned().collect()
	}

	/// Get error count.
	pub fn error_count(&self) -> usize {
		self.diagnostics.read().iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).count()
	}

	/// Get warning count.
	pub fn warning_count(&self) -> usize {
		self.diagnostics
			.read()
			.iter()
			.filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
			.count()
	}

	/// Check if there are any errors.
	pub fn has_errors(&self) -> bool {
		self.error_count() > 0
	}

	/// Check if there are any warnings.
	pub fn has_warnings(&self) -> bool {
		self.warning_count() > 0
	}
}
