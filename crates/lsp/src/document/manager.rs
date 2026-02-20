//! Document state manager for LSP.
//!
//! Manages LSP state across all open documents, including diagnostics,
//! version tracking, and progress operations.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use lsp_types::{Diagnostic, DiagnosticSeverity, ProgressParams, Uri};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::debug;

use super::progress::ProgressItem;
use super::state::DocumentState;
use super::{DiagnosticsEvent, DiagnosticsEventReceiver, DiagnosticsEventSender};
use crate::client::LanguageServerId;

/// Manager for document LSP state across all open documents.
///
/// This can be shared across the editor to track LSP state for all buffers.
pub struct DocumentStateManager {
	/// Document states keyed by URI string.
	documents: RwLock<HashMap<String, DocumentState>>,
	/// Optional sender for diagnostic events.
	event_sender: Option<DiagnosticsEventSender>,
	/// Global version counter for tracking any diagnostic change.
	diagnostics_version: AtomicU64,
	/// Monotonic counter for document open-session generations. Each
	/// [`mark_opened`](Self::mark_opened) call bumps this and assigns
	/// the new value to the document, ensuring barriers from a previous
	/// session are distinguishable even after close+reopen.
	doc_generation: AtomicU64,
	/// Monotonic counter for diagnostics touch ordering (LRU eviction).
	diag_touch_counter: AtomicU64,
	/// Maximum number of closed-document diagnostic entries retained.
	/// Only entries with `opened == false` count toward this limit.
	max_closed_diagnostic_entries: usize,
	/// Active progress operations keyed by (server_id, token).
	progress: RwLock<HashMap<(LanguageServerId, String), ProgressItem>>,
}

impl std::fmt::Debug for DocumentStateManager {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DocumentStateManager")
			.field("documents", &self.documents)
			.field("has_event_sender", &self.event_sender.is_some())
			.field("diagnostics_version", &self.diagnostics_version)
			.field("doc_generation", &self.doc_generation)
			.field("progress_count", &self.progress.read().len())
			.finish()
	}
}

impl Default for DocumentStateManager {
	fn default() -> Self {
		Self::new()
	}
}

impl DocumentStateManager {
	fn normalize_uri(&self, uri: &Uri) -> Uri {
		if let Some(path) = crate::path_from_uri(uri)
			&& let Some(normalized) = crate::uri_from_path(&path)
		{
			return normalized;
		}
		uri.clone()
	}

	fn uri_key(&self, uri: &Uri) -> String {
		self.normalize_uri(uri).to_string()
	}

	/// Default maximum number of closed-document diagnostic entries.
	const DEFAULT_MAX_CLOSED_DIAGNOSTIC_ENTRIES: usize = 512;

	/// Create a new empty manager.
	pub fn new() -> Self {
		Self {
			documents: RwLock::new(HashMap::new()),
			event_sender: None,
			diagnostics_version: AtomicU64::new(0),
			doc_generation: AtomicU64::new(0),
			diag_touch_counter: AtomicU64::new(0),
			max_closed_diagnostic_entries: Self::DEFAULT_MAX_CLOSED_DIAGNOSTIC_ENTRIES,
			progress: RwLock::new(HashMap::new()),
		}
	}

	/// Create a new manager with an event channel.
	///
	/// Returns the manager and a receiver for diagnostic events.
	pub fn with_events() -> (Self, DiagnosticsEventReceiver) {
		let (sender, receiver) = mpsc::unbounded_channel();
		let manager = Self {
			documents: RwLock::new(HashMap::new()),
			event_sender: Some(sender),
			diagnostics_version: AtomicU64::new(0),
			doc_generation: AtomicU64::new(0),
			diag_touch_counter: AtomicU64::new(0),
			max_closed_diagnostic_entries: Self::DEFAULT_MAX_CLOSED_DIAGNOSTIC_ENTRIES,
			progress: RwLock::new(HashMap::new()),
		};
		(manager, receiver)
	}

	/// Sets the maximum number of closed-document diagnostic entries.
	#[cfg(test)]
	pub fn set_max_closed_diagnostic_entries(&mut self, max: usize) {
		self.max_closed_diagnostic_entries = max;
	}

	/// Get the current diagnostics version.
	///
	/// This counter increments every time any document's diagnostics change.
	/// Useful for detecting if a re-render is needed.
	pub fn diagnostics_version(&self) -> u64 {
		self.diagnostics_version.load(Ordering::Relaxed)
	}

	/// Get document state by file path.
	pub fn get_by_path(&self, path: &Path) -> Option<Uri> {
		let uri = crate::uri_from_path(path)?;
		let key = self.uri_key(&uri);
		let docs = self.documents.read();
		if docs.contains_key(&key) { Some(uri) } else { None }
	}

	/// Get document state by URI.
	pub fn contains(&self, uri: &Uri) -> bool {
		let key = self.uri_key(uri);
		self.documents.read().contains_key(&key)
	}

	/// Register a document.
	pub fn register(&self, path: &Path, language_id: Option<&str>) -> Option<Uri> {
		let uri = crate::uri_from_path(path)?;
		let key = self.uri_key(&uri);

		let mut docs = self.documents.write();
		let state = docs.entry(key).or_insert_with(|| DocumentState::from_uri(uri.clone()));

		if let Some(lang) = language_id {
			state.set_language_id(lang);
		}

		Some(uri)
	}

	/// Unregister a document.
	pub fn unregister(&self, uri: &Uri) {
		let key = self.uri_key(uri);
		self.documents.write().remove(&key);
	}

	/// Bumps and returns the next diagnostics touch sequence.
	fn next_diag_touch_seq(&self) -> u64 {
		self.diag_touch_counter.fetch_add(1, Ordering::Relaxed) + 1
	}

	/// Evicts the least-recently-touched closed-document entry if over the cap.
	///
	/// Only entries with `opened == false` count toward the limit.
	/// Never evicts entries with `opened == true`.
	fn evict_closed_entries_if_needed(docs: &mut HashMap<String, DocumentState>, max: usize) {
		let closed_count = docs.values().filter(|s| !s.is_opened()).count();
		if closed_count <= max {
			return;
		}
		let evict_count = closed_count - max;
		for _ in 0..evict_count {
			let lru_key = docs
				.iter()
				.filter(|(_, s)| !s.is_opened())
				.min_by_key(|(_, s)| s.diag_touch_seq())
				.map(|(k, _)| k.clone());
			if let Some(key) = lru_key {
				docs.remove(&key);
			}
		}
	}

	/// Updates diagnostics for a document.
	///
	/// Creates document state on-demand if the document isn't registered,
	/// enabling project-wide diagnostics from LSP servers.
	///
	/// For closed documents (`opened == false`):
	/// * Empty diagnostics remove the entry entirely (no tombstones).
	/// * Non-empty diagnostics are retained up to `max_closed_diagnostic_entries`,
	///   evicting the least-recently-touched closed entry when over the cap.
	pub fn update_diagnostics(&self, uri: &Uri, diagnostics: Vec<Diagnostic>, version: Option<i32>) {
		let error_count = diagnostics.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).count();
		let warning_count = diagnostics.iter().filter(|d| d.severity == Some(DiagnosticSeverity::WARNING)).count();
		let touch_seq = self.next_diag_touch_seq();

		let uri_key = self.uri_key(uri);

		// Try read lock first for the common case (opened documents)
		{
			let docs = self.documents.read();
			if let Some(state) = docs.get(&uri_key) {
				if state.is_opened() {
					state.set_diagnostics(diagnostics);
					state.set_diag_touch_seq(touch_seq);
					if let Some(version) = version {
						state.record_diagnostics_version(version);
					}
					self.diagnostics_version.fetch_add(1, Ordering::Relaxed);
					self.send_diagnostics_event(uri, error_count, warning_count);
					return;
				}
			}
		}

		// Closed or unregistered document — needs write lock for eviction/removal
		{
			let mut docs = self.documents.write();

			// Empty diagnostics for an on-demand closed entry: remove entirely.
			// Only remove if the entry was never version-tracked (version == 0),
			// indicating it was created on-demand for project-wide diagnostics
			// rather than through explicit `register`.
			if diagnostics.is_empty() {
				if let Some(state) = docs.get(&uri_key) {
					if !state.is_opened() && state.version() == 0 {
						docs.remove(&uri_key);
						self.diagnostics_version.fetch_add(1, Ordering::Relaxed);
						self.send_diagnostics_event(uri, 0, 0);
						return;
					}
				}
			}

			let state = if let Some(state) = docs.get(&uri_key) {
				state
			} else {
				let state = DocumentState::from_uri(self.normalize_uri(uri));
				docs.insert(uri_key.clone(), state);
				docs.get(&uri_key).expect("state just inserted")
			};

			state.set_diagnostics(diagnostics);
			state.set_diag_touch_seq(touch_seq);
			if let Some(version) = version {
				state.record_diagnostics_version(version);
			}

			Self::evict_closed_entries_if_needed(&mut docs, self.max_closed_diagnostic_entries);
		}

		self.diagnostics_version.fetch_add(1, Ordering::Relaxed);
		self.send_diagnostics_event(uri, error_count, warning_count);
	}

	fn send_diagnostics_event(&self, uri: &Uri, error_count: usize, warning_count: usize) {
		let has_sender = self.event_sender.is_some();
		let path = crate::path_from_uri(uri);
		debug!(uri = uri.as_str(), ?path, error_count, warning_count, has_sender, "Sending diagnostics event");
		if let Some(ref sender) = self.event_sender
			&& let Some(path) = path
		{
			let _ = sender.send(DiagnosticsEvent {
				path,
				error_count,
				warning_count,
			});
		}
	}

	/// Get diagnostics for a document.
	pub fn get_diagnostics(&self, uri: &Uri) -> Vec<Diagnostic> {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.diagnostics()).unwrap_or_default()
	}

	/// Increment version for a document and return the new version.
	pub fn increment_version(&self, uri: &Uri) -> Option<i32> {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.increment_version())
	}

	/// Reserve the next sync version and mark it pending.
	pub fn queue_change(&self, uri: &Uri) -> Option<i32> {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.next_version())
	}

	/// Acknowledge a pending version; returns false on mismatch.
	pub fn ack_change(&self, uri: &Uri, version: i32) -> bool {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).is_some_and(|s| s.ack_version(version))
	}

	/// Get version for a document.
	pub fn get_version(&self, uri: &Uri) -> Option<i32> {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.version())
	}

	/// Get the last acked version for a document.
	pub fn acked_version(&self, uri: &Uri) -> Option<i32> {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.acked_version())
	}

	/// Mark a document as opened with a language server.
	pub fn set_opened(&self, uri: &Uri, opened: bool) {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		if let Some(state) = docs.get(&key) {
			state.set_opened(opened);
		}
	}

	/// Mark a document as opened and reset sync state.
	///
	/// Assigns a new globally unique generation so barriers from previous
	/// sessions are invalidated.
	pub fn mark_opened(&self, uri: &Uri, version: i32) {
		let generation = self.doc_generation.fetch_add(1, Ordering::Relaxed) + 1;
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		if let Some(state) = docs.get(&key) {
			state.mark_opened(version, generation);
		}
	}

	/// Returns the current session generation for a document, or `None` if
	/// the document is not registered.
	pub fn doc_generation(&self, uri: &Uri) -> Option<u64> {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.generation())
	}

	/// Mark a document as requiring a full sync.
	pub fn mark_force_full_sync(&self, uri: &Uri) {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		if let Some(state) = docs.get(&key) {
			state.mark_force_full_sync();
		}
	}

	/// Returns true if a document requires a full sync and clears the flag.
	pub fn take_force_full_sync_by_uri(&self, uri: &Uri) -> bool {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).is_some_and(|s| s.take_force_full_sync())
	}

	/// Returns and clears all documents marked for full sync.
	pub fn take_force_full_sync_uris(&self) -> Vec<Uri> {
		let docs = self.documents.read();
		let mut uris = Vec::new();
		for state in docs.values() {
			if state.take_force_full_sync() {
				uris.push(state.uri().clone());
			}
		}
		uris
	}

	/// Check if a document is opened with a language server.
	pub fn is_opened(&self, uri: &Uri) -> bool {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).is_some_and(|s| s.is_opened())
	}

	/// Get all documents with errors.
	pub fn documents_with_errors(&self) -> Vec<Uri> {
		self.documents
			.read()
			.iter()
			.filter(|(_, s)| s.has_errors())
			.map(|(_, s)| s.uri().clone())
			.collect()
	}

	/// Get total error count across all documents.
	pub fn total_error_count(&self) -> usize {
		self.documents.read().values().map(|s| s.error_count()).sum()
	}

	/// Get total warning count across all documents.
	pub fn total_warning_count(&self) -> usize {
		self.documents.read().values().map(|s| s.warning_count()).sum()
	}

	/// Handle a progress notification from a language server.
	pub fn update_progress(&self, server_id: LanguageServerId, params: ProgressParams) {
		use lsp_types::WorkDoneProgress;

		let token_key = match &params.token {
			lsp_types::NumberOrString::Number(n) => n.to_string(),
			lsp_types::NumberOrString::String(s) => s.clone(),
		};
		let key = (server_id, token_key);

		match params.value {
			lsp_types::ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(begin)) => {
				let item = ProgressItem {
					server_id,
					token: params.token,
					title: begin.title,
					message: begin.message,
					percentage: begin.percentage,
					started_at: Instant::now(),
				};
				debug!(
					%server_id,
					title = %item.title,
					"Progress started"
				);
				self.progress.write().insert(key, item);
			}
			lsp_types::ProgressParamsValue::WorkDone(WorkDoneProgress::Report(report)) => {
				if let Some(item) = self.progress.write().get_mut(&key) {
					if report.message.is_some() {
						item.message = report.message;
					}
					if report.percentage.is_some() {
						item.percentage = report.percentage;
					}
				}
			}
			lsp_types::ProgressParamsValue::WorkDone(WorkDoneProgress::End(end)) => {
				if let Some(item) = self.progress.write().remove(&key) {
					debug!(
						%server_id,
						title = %item.title,
						message = ?end.message,
						elapsed_ms = item.started_at.elapsed().as_millis(),
						"Progress ended"
					);
				}
			}
		}
	}

	/// Get all active progress items.
	pub fn active_progress(&self) -> Vec<ProgressItem> {
		self.progress.read().values().cloned().collect()
	}

	/// Get the current progress status message, if any.
	///
	/// Returns the most recently started progress item's title and message.
	pub fn progress_status(&self) -> Option<String> {
		let progress = self.progress.read();
		if progress.is_empty() {
			return None;
		}

		// Find the most recently started item
		progress.values().max_by_key(|p| p.started_at).map(|item| {
			if let Some(ref msg) = item.message {
				format!("{}: {}", item.title, msg)
			} else if let Some(pct) = item.percentage {
				format!("{} ({}%)", item.title, pct)
			} else {
				item.title.clone()
			}
		})
	}

	/// Check if there are any active progress operations.
	pub fn has_progress(&self) -> bool {
		!self.progress.read().is_empty()
	}

	/// Clear all progress for a specific server (e.g., when server crashes).
	pub fn clear_server_progress(&self, server_id: LanguageServerId) {
		self.progress.write().retain(|(sid, _), _| *sid != server_id);
	}

	/// Returns the number of pending changes for a document.
	#[cfg(test)]
	pub fn pending_change_count(&self, uri: &Uri) -> usize {
		let key = self.uri_key(uri);
		let docs = self.documents.read();
		docs.get(&key).map(|s| s.pending_versions_len()).unwrap_or(0)
	}
}
