//! LSP document state tracking.
//!
//! This module provides types for tracking LSP-related state for documents,
//! including version numbers, diagnostics, and language server associations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};

use lsp_types::{Diagnostic, DiagnosticSeverity, Url};
use parking_lot::RwLock;

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
#[derive(Debug, Default)]
pub struct DocumentStateManager {
	/// Document states keyed by URI string.
	documents: RwLock<HashMap<String, DocumentState>>,
}

impl DocumentStateManager {
	/// Create a new empty manager.
	pub fn new() -> Self {
		Self::default()
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
		let docs = self.documents.read();
		if let Some(state) = docs.get(&uri.to_string()) {
			state.set_diagnostics(diagnostics);
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

		// Register a document
		let path = PathBuf::from("/test.rs");
		manager.register(&path, Some("rust"));

		// Check it exists
		assert!(manager.contains(&uri));

		// Update diagnostics
		let diagnostics = vec![make_diagnostic(DiagnosticSeverity::ERROR, "test error")];
		manager.update_diagnostics(&uri, diagnostics);

		assert_eq!(manager.get_diagnostics(&uri).len(), 1);
		assert_eq!(manager.total_error_count(), 1);

		// Unregister
		manager.unregister(&uri);
		assert!(!manager.contains(&uri));
	}
}
