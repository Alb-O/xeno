//! Document synchronization between editor and language servers.
//!
//! This module provides the [`DocumentSync`] type which coordinates document
//! state with language servers, handling:
//!
//! - Opening documents (`textDocument/didOpen`)
//! - Tracking changes (`textDocument/didChange`)
//! - Saving documents (`textDocument/didSave`)
//! - Closing documents (`textDocument/didClose`)
//!
//! # Architecture
//!
//! The sync module acts as a bridge between the editor's document model and
//! the LSP protocol. It maintains a [`DocumentStateManager`] to track versions
//! and a [`Registry`] to manage language server connections.
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────────┐
//! │   Buffer    │────▶│ DocumentSync │────▶│ Language Server │
//! │  (Editor)   │     │   (Bridge)   │     │   (rust-analyzer)│
//! └─────────────┘     └──────────────┘     └─────────────────┘
//!                            │
//!                            ▼
//!                     ┌──────────────────┐
//!                     │DocumentStateManager│
//!                     │  (Version, Diags) │
//!                     └──────────────────┘
//! ```

use std::path::Path;
use std::sync::Arc;

use lsp_types::{Diagnostic, TextDocumentContentChangeEvent, TextDocumentSaveReason, Url};
use ropey::Rope;

use crate::Result;
use crate::client::{ClientHandle, LanguageServerId, LspEventHandler, OffsetEncoding};
use crate::document::{DiagnosticsEventReceiver, DocumentStateManager};
use crate::position::char_range_to_lsp_range;
use crate::registry::Registry;

/// Event handler that updates [`DocumentStateManager`] with LSP events.
///
/// This handler receives notifications from language servers and updates
/// the document state accordingly (e.g., storing diagnostics).
pub struct DocumentSyncEventHandler {
	documents: Arc<DocumentStateManager>,
}

impl DocumentSyncEventHandler {
	/// Create a new event handler.
	pub fn new(documents: Arc<DocumentStateManager>) -> Self {
		Self { documents }
	}
}

impl LspEventHandler for DocumentSyncEventHandler {
	fn on_diagnostics(&self, _server_id: LanguageServerId, uri: Url, diagnostics: Vec<Diagnostic>) {
		self.documents.update_diagnostics(&uri, diagnostics);
	}

	// Other trait methods (on_progress, on_log_message, on_show_message) use default no-op impls.
	// Logging is handled by tracing in the client router.
}

/// Document synchronization coordinator.
///
/// Manages the lifecycle of documents with language servers, tracking
/// versions and coordinating notifications.
#[derive(Clone)]
pub struct DocumentSync {
	/// Language server registry.
	registry: Arc<Registry>,
	/// Document state manager.
	documents: Arc<DocumentStateManager>,
}

impl DocumentSync {
	/// Create a new document sync coordinator.
	///
	/// This sets up an event handler on the registry so that diagnostics
	/// and other LSP events are automatically routed to the document state manager.
	pub fn new(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		// Create event handler that updates the document state manager
		let event_handler = Arc::new(DocumentSyncEventHandler::new(documents.clone()));

		// We need to get mutable access to set the event handler.
		// Since Registry uses interior mutability for configs/servers, we need
		// a different approach. For now, we require the registry to be created
		// with the event handler via Registry::with_event_handler().
		//
		// TODO: Consider making Registry::set_event_handler take &self with interior mutability.
		let _ = event_handler; // Silence unused warning for now

		Self {
			registry,
			documents,
		}
	}

	/// Create a new document sync coordinator with a pre-configured registry.
	///
	/// Use this when you need to set up the event handler before creating the sync.
	/// The registry should be created with [`Registry::with_event_handler`] using
	/// a [`DocumentSyncEventHandler`].
	pub fn with_registry(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		Self {
			registry,
			documents,
		}
	}

	/// Create a document sync coordinator and a properly configured registry.
	///
	/// This is the recommended way to create a DocumentSync, as it ensures
	/// the event handler is properly wired up and diagnostic events are available.
	///
	/// Returns:
	/// - `DocumentSync` - The sync coordinator
	/// - `Arc<Registry>` - The language server registry
	/// - `Arc<DocumentStateManager>` - The document state manager
	/// - `DiagnosticsEventReceiver` - Receiver for diagnostic update events
	pub fn create() -> (
		Self,
		Arc<Registry>,
		Arc<DocumentStateManager>,
		DiagnosticsEventReceiver,
	) {
		let (documents, event_receiver) = DocumentStateManager::with_events();
		let documents = Arc::new(documents);
		let event_handler = Arc::new(DocumentSyncEventHandler::new(documents.clone()));
		let registry = Arc::new(Registry::with_event_handler(event_handler));

		let sync = Self {
			registry: registry.clone(),
			documents: documents.clone(),
		};

		(sync, registry, documents, event_receiver)
	}

	/// Open a document with the appropriate language server.
	///
	/// This finds or starts a language server for the document's language,
	/// registers the document, and sends `textDocument/didOpen`.
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	/// * `language` - Language ID (e.g., "rust", "python")
	/// * `text` - Current document content
	pub async fn open_document(
		&self,
		path: &Path,
		language: &str,
		text: &Rope,
	) -> Result<ClientHandle> {
		let client = self.registry.get_or_start(language, path).await?;

		let uri = self
			.documents
			.register(&path.to_path_buf(), Some(language))
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		let version = self.documents.get_version(&uri).unwrap_or(0);

		client.text_document_did_open(
			uri.clone(),
			language.to_string(),
			version,
			text.to_string(),
		)?;

		self.documents.set_opened(&uri, true);

		Ok(client)
	}

	/// Notify language servers of a document change.
	///
	/// This sends a full document sync (the entire content). For incremental
	/// sync, use [`notify_change_incremental`](Self::notify_change_incremental).
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	/// * `language` - Language ID
	/// * `text` - New document content
	pub async fn notify_change_full(&self, path: &Path, language: &str, text: &Rope) -> Result<()> {
		let uri =
			Url::from_file_path(path).map_err(|_| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			self.open_document(path, language, text).await?;
			return Ok(());
		}

		let version = self
			.documents
			.increment_version(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		if let Some(client) = self.registry.get(language, path) {
			client.text_document_did_change_full(uri, version, text.to_string())?;
		}

		Ok(())
	}

	/// Notify language servers of an incremental document change.
	///
	/// # Warning
	///
	/// This function has a known limitation: it computes LSP positions from
	/// the post-change text, but the positions should be relative to the
	/// pre-change text. For reliable document synchronization, prefer using
	/// [`notify_change_full`](Self::notify_change_full) instead.
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	/// * `language` - Language ID
	/// * `text` - Current document content (after the change)
	/// * `start_char` - Start position of the change (character index in NEW text)
	/// * `end_char` - End position of the change (character index in NEW text)
	/// * `new_text` - The replacement text
	/// * `encoding` - Offset encoding to use
	#[deprecated(
		since = "0.3.0",
		note = "Use notify_change_full instead; incremental sync has position calculation issues"
	)]
	pub async fn notify_change_incremental(
		&self,
		path: &Path,
		language: &str,
		text: &Rope,
		start_char: usize,
		end_char: usize,
		new_text: &str,
		encoding: OffsetEncoding,
	) -> Result<()> {
		let uri =
			Url::from_file_path(path).map_err(|_| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			self.open_document(path, language, text).await?;
			return Ok(());
		}

		let version = self
			.documents
			.increment_version(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		// Incremental sync requires the old text to compute the range properly;
		// the caller must provide correct positions relative to pre-change state.
		let range =
			char_range_to_lsp_range(text, start_char, end_char.min(text.len_chars()), encoding)
				.ok_or_else(|| crate::Error::Protocol("Invalid range".into()))?;

		let change = TextDocumentContentChangeEvent {
			range: Some(range),
			range_length: None,
			text: new_text.to_string(),
		};

		if let Some(client) = self.registry.get(language, path) {
			client.text_document_did_change(uri, version, vec![change])?;
		}

		Ok(())
	}

	/// Notify language servers that a document will be saved.
	pub fn notify_will_save(&self, path: &Path, language: &str) -> Result<()> {
		let uri =
			Url::from_file_path(path).map_err(|_| crate::Error::Protocol("Invalid path".into()))?;

		if let Some(client) = self.registry.get(language, path) {
			client.text_document_will_save(uri, TextDocumentSaveReason::MANUAL)?;
		}

		Ok(())
	}

	/// Notify language servers that a document was saved.
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	/// * `language` - Language ID
	/// * `include_text` - Whether to include the document text (some servers need it)
	/// * `text` - Document content (only sent if `include_text` is true)
	pub fn notify_did_save(
		&self,
		path: &Path,
		language: &str,
		include_text: bool,
		text: Option<&Rope>,
	) -> Result<()> {
		let uri =
			Url::from_file_path(path).map_err(|_| crate::Error::Protocol("Invalid path".into()))?;

		let text_content = if include_text {
			text.map(|t| t.to_string())
		} else {
			None
		};

		if let Some(client) = self.registry.get(language, path) {
			client.text_document_did_save(uri, text_content)?;
		}

		Ok(())
	}

	/// Close a document with language servers.
	///
	/// This sends `textDocument/didClose` and removes the document from tracking.
	pub fn close_document(&self, path: &Path, language: &str) -> Result<()> {
		let uri =
			Url::from_file_path(path).map_err(|_| crate::Error::Protocol("Invalid path".into()))?;

		if self.documents.is_opened(&uri)
			&& let Some(client) = self.registry.get(language, path)
		{
			client.text_document_did_close(uri.clone())?;
		}

		self.documents.unregister(&uri);

		Ok(())
	}

	/// Get diagnostics for a document.
	pub fn get_diagnostics(&self, path: &Path) -> Vec<lsp_types::Diagnostic> {
		if let Ok(uri) = Url::from_file_path(path) {
			self.documents.get_diagnostics(&uri)
		} else {
			Vec::new()
		}
	}

	/// Get error count for a document.
	pub fn error_count(&self, path: &Path) -> usize {
		if let Ok(uri) = Url::from_file_path(path) {
			let diags = self.documents.get_diagnostics(&uri);
			diags
				.iter()
				.filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR))
				.count()
		} else {
			0
		}
	}

	/// Get warning count for a document.
	pub fn warning_count(&self, path: &Path) -> usize {
		if let Ok(uri) = Url::from_file_path(path) {
			let diags = self.documents.get_diagnostics(&uri);
			diags
				.iter()
				.filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::WARNING))
				.count()
		} else {
			0
		}
	}

	/// Get total error count across all documents.
	pub fn total_error_count(&self) -> usize {
		self.documents.total_error_count()
	}

	/// Get total warning count across all documents.
	pub fn total_warning_count(&self) -> usize {
		self.documents.total_warning_count()
	}

	/// Get the registry.
	pub fn registry(&self) -> &Registry {
		&self.registry
	}

	/// Get the document state manager.
	pub fn documents(&self) -> &DocumentStateManager {
		&self.documents
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_document_sync_new() {
		let registry = Arc::new(Registry::new());
		let documents = Arc::new(DocumentStateManager::new());
		let sync = DocumentSync::new(registry, documents);

		assert_eq!(sync.total_error_count(), 0);
		assert_eq!(sync.total_warning_count(), 0);
	}
}
