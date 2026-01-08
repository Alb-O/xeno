//! LSP document state tracking.
//!
//! This module provides types for tracking LSP-related state for documents,
//! including version numbers, diagnostics, and language server associations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::Instant;

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, ProgressParams, Url};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::debug;

use crate::client::LanguageServerId;

/// Event emitted when diagnostics are updated for a document.
#[derive(Debug, Clone)]
pub struct DiagnosticsEvent {
	/// Path to the document (derived from URI).
	pub path: PathBuf,
	/// Number of error diagnostics.
	pub error_count: usize,
	/// Number of warning diagnostics.
	pub warning_count: usize,
}

/// Sender for diagnostic events.
pub type DiagnosticsEventSender = mpsc::UnboundedSender<DiagnosticsEvent>;

/// Receiver for diagnostic events.
pub type DiagnosticsEventReceiver = mpsc::UnboundedReceiver<DiagnosticsEvent>;

/// An active progress operation from a language server.
#[derive(Debug, Clone)]
pub struct ProgressItem {
	/// Server that reported this progress.
	pub server_id: LanguageServerId,
	/// Progress token for tracking.
	pub token: NumberOrString,
	/// Title of the operation (e.g., "Indexing").
	pub title: String,
	/// Optional message with more details.
	pub message: Option<String>,
	/// Optional percentage (0-100).
	pub percentage: Option<u32>,
	/// When this progress started.
	pub started_at: Instant,
}

/// LSP state for a single document.
///
/// Tracks version number for incremental sync, diagnostics, and other
/// LSP-related metadata.
#[derive(Debug)]
pub struct DocumentState {
	/// Document URI (derived from file path).
	uri: Url,
	/// Document version for LSP sync. Incremented on each change.
	version: AtomicI32,
	/// Whether the document has been opened with the language server.
	opened: RwLock<bool>,
	/// Current diagnostics from the language server.
	diagnostics: RwLock<Vec<Diagnostic>>,
	/// Language ID for the document (e.g., "rust", "python").
	language_id: RwLock<Option<String>>,
}

impl DocumentState {
	/// Create a new document state from a file path.
	///
	/// Returns `None` if the path cannot be converted to a URL.
	pub fn new(path: &PathBuf) -> Option<Self> {
		let uri = Url::from_file_path(path).ok()?;
		Some(Self {
			uri,
			version: AtomicI32::new(0),
			opened: RwLock::new(false),
			diagnostics: RwLock::new(Vec::new()),
			language_id: RwLock::new(None),
		})
	}

	/// Create a document state from a URI directly.
	pub fn from_uri(uri: Url) -> Self {
		Self {
			uri,
			version: AtomicI32::new(0),
			opened: RwLock::new(false),
			diagnostics: RwLock::new(Vec::new()),
			language_id: RwLock::new(None),
		}
	}

	/// Get the document URI.
	pub fn uri(&self) -> &Url {
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

	/// Get the language ID.
	pub fn language_id(&self) -> Option<String> {
		self.language_id.read().clone()
	}

	/// Set the language ID.
	pub fn set_language_id(&self, lang: impl Into<String>) {
		*self.language_id.write() = Some(lang.into());
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
		self.diagnostics
			.read()
			.iter()
			.filter(|d| d.severity == Some(severity))
			.cloned()
			.collect()
	}

	/// Get error count.
	pub fn error_count(&self) -> usize {
		self.diagnostics
			.read()
			.iter()
			.filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
			.count()
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
	/// Active progress operations keyed by (server_id, token).
	progress: RwLock<HashMap<(u64, String), ProgressItem>>,
}

impl std::fmt::Debug for DocumentStateManager {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DocumentStateManager")
			.field("documents", &self.documents)
			.field("has_event_sender", &self.event_sender.is_some())
			.field("diagnostics_version", &self.diagnostics_version)
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
	/// Create a new empty manager.
	pub fn new() -> Self {
		Self {
			documents: RwLock::new(HashMap::new()),
			event_sender: None,
			diagnostics_version: AtomicU64::new(0),
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
			progress: RwLock::new(HashMap::new()),
		};
		(manager, receiver)
	}

	/// Get the current diagnostics version.
	///
	/// This counter increments every time any document's diagnostics change.
	/// Useful for detecting if a re-render is needed.
	pub fn diagnostics_version(&self) -> u64 {
		self.diagnostics_version.load(Ordering::Relaxed)
	}

	/// Get document state by file path.
	pub fn get_by_path(&self, path: &PathBuf) -> Option<Url> {
		let uri = Url::from_file_path(path).ok()?;
		let key = uri.to_string();
		let docs = self.documents.read();
		if docs.contains_key(&key) {
			Some(uri)
		} else {
			None
		}
	}

	/// Get document state by URI.
	pub fn contains(&self, uri: &Url) -> bool {
		self.documents.read().contains_key(&uri.to_string())
	}

	/// Register a document.
	pub fn register(&self, path: &PathBuf, language_id: Option<&str>) -> Option<Url> {
		let uri = Url::from_file_path(path).ok()?;
		let key = uri.to_string();

		let mut docs = self.documents.write();
		let state = docs
			.entry(key)
			.or_insert_with(|| DocumentState::from_uri(uri.clone()));

		if let Some(lang) = language_id {
			state.set_language_id(lang);
		}

		Some(uri)
	}

	/// Unregister a document.
	pub fn unregister(&self, uri: &Url) {
		self.documents.write().remove(&uri.to_string());
	}

	/// Update diagnostics for a document.
	pub fn update_diagnostics(&self, uri: &Url, diagnostics: Vec<Diagnostic>) {
		// Count errors and warnings
		let error_count = diagnostics
			.iter()
			.filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
			.count();
		let warning_count = diagnostics
			.iter()
			.filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
			.count();

		// Update the document state
		let docs = self.documents.read();
		if let Some(state) = docs.get(&uri.to_string()) {
			state.set_diagnostics(diagnostics);
		}

		// Increment version counter
		self.diagnostics_version.fetch_add(1, Ordering::Relaxed);

		// Send event if we have a sender
		if let Some(ref sender) = self.event_sender
			&& let Ok(path) = uri.to_file_path()
		{
			let _ = sender.send(DiagnosticsEvent {
				path,
				error_count,
				warning_count,
			});
		}
	}

	/// Get diagnostics for a document.
	pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
		let docs = self.documents.read();
		docs.get(&uri.to_string())
			.map(|s| s.diagnostics())
			.unwrap_or_default()
	}

	/// Increment version for a document and return the new version.
	pub fn increment_version(&self, uri: &Url) -> Option<i32> {
		let docs = self.documents.read();
		docs.get(&uri.to_string()).map(|s| s.increment_version())
	}

	/// Get version for a document.
	pub fn get_version(&self, uri: &Url) -> Option<i32> {
		let docs = self.documents.read();
		docs.get(&uri.to_string()).map(|s| s.version())
	}

	/// Mark a document as opened with a language server.
	pub fn set_opened(&self, uri: &Url, opened: bool) {
		let docs = self.documents.read();
		if let Some(state) = docs.get(&uri.to_string()) {
			state.set_opened(opened);
		}
	}

	/// Check if a document is opened with a language server.
	pub fn is_opened(&self, uri: &Url) -> bool {
		let docs = self.documents.read();
		docs.get(&uri.to_string())
			.map(|s| s.is_opened())
			.unwrap_or(false)
	}

	/// Get all documents with errors.
	pub fn documents_with_errors(&self) -> Vec<Url> {
		self.documents
			.read()
			.iter()
			.filter(|(_, s)| s.has_errors())
			.map(|(_, s)| s.uri().clone())
			.collect()
	}

	/// Get total error count across all documents.
	pub fn total_error_count(&self) -> usize {
		self.documents
			.read()
			.values()
			.map(|s| s.error_count())
			.sum()
	}

	/// Get total warning count across all documents.
	pub fn total_warning_count(&self) -> usize {
		self.documents
			.read()
			.values()
			.map(|s| s.warning_count())
			.sum()
	}

	/// Handle a progress notification from a language server.
	pub fn update_progress(&self, server_id: LanguageServerId, params: ProgressParams) {
		use lsp_types::WorkDoneProgress;

		let token_key = match &params.token {
			NumberOrString::Number(n) => n.to_string(),
			NumberOrString::String(s) => s.clone(),
		};
		let key = (server_id.0, token_key);

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
					server_id = server_id.0,
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
						server_id = server_id.0,
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
		self.progress
			.write()
			.retain(|(sid, _), _| *sid != server_id.0);
	}
}

#[cfg(test)]
mod tests {
	use lsp_types::Range;

	use super::*;

	fn make_diagnostic(severity: DiagnosticSeverity, message: &str) -> Diagnostic {
		Diagnostic {
			range: Range::default(),
			severity: Some(severity),
			code: None,
			code_description: None,
			source: Some("test".into()),
			message: message.into(),
			related_information: None,
			tags: None,
			data: None,
		}
	}

	#[test]
	fn test_document_state_version() {
		let uri = Url::parse("file:///test.rs").unwrap();
		let state = DocumentState::from_uri(uri);

		assert_eq!(state.version(), 0);
		assert_eq!(state.increment_version(), 1);
		assert_eq!(state.increment_version(), 2);
		assert_eq!(state.version(), 2);
	}

	#[test]
	fn test_document_state_diagnostics() {
		let uri = Url::parse("file:///test.rs").unwrap();
		let state = DocumentState::from_uri(uri);

		assert!(!state.has_errors());
		assert!(!state.has_warnings());

		let diagnostics = vec![
			make_diagnostic(DiagnosticSeverity::ERROR, "error 1"),
			make_diagnostic(DiagnosticSeverity::ERROR, "error 2"),
			make_diagnostic(DiagnosticSeverity::WARNING, "warning 1"),
		];
		state.set_diagnostics(diagnostics);

		assert!(state.has_errors());
		assert!(state.has_warnings());
		assert_eq!(state.error_count(), 2);
		assert_eq!(state.warning_count(), 1);
	}

	#[test]
	fn test_document_state_manager() {
		let manager = DocumentStateManager::new();
		let uri = Url::parse("file:///test.rs").unwrap();

		let path = PathBuf::from("/test.rs");
		manager.register(&path, Some("rust"));
		assert!(manager.contains(&uri));

		let diagnostics = vec![make_diagnostic(DiagnosticSeverity::ERROR, "test error")];
		manager.update_diagnostics(&uri, diagnostics);
		assert_eq!(manager.get_diagnostics(&uri).len(), 1);
		assert_eq!(manager.total_error_count(), 1);

		manager.unregister(&uri);
		assert!(!manager.contains(&uri));
	}
}
