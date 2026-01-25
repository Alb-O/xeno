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
//! ┌────────────┐     ┌──────────────┐     ┌─────────────────┐
//! │   Buffer   │────▶│ DocumentSync │────▶│ Language Server │
//! │  (Editor)  │     │   (Bridge)   │     │ (rust-analyzer) │
//! └────────────┘     └──────────────┘     └─────────────────┘
//!                            │
//!                            ▼
//!                 ┌────────────────────┐
//!                 │DocumentStateManager│
//!                 │  (Version, Diags)  │
//!                 └────────────────────┘
//! ```

use std::path::Path;
use std::sync::Arc;

use futures::channel::oneshot;
use lsp_types::{Diagnostic, TextDocumentContentChangeEvent, TextDocumentSaveReason, Uri};
use ropey::Rope;
use xeno_primitives::lsp::LspDocumentChange;

use crate::Result;
use crate::client::{ClientHandle, LanguageServerId, LspEventHandler};
use crate::document::{DiagnosticsEventReceiver, DocumentStateManager};
use crate::registry::Registry;

/// Event handler that updates [`DocumentStateManager`] with LSP events.
///
/// This handler receives notifications from language servers and updates
/// the document state accordingly (e.g., storing diagnostics).
pub struct DocumentSyncEventHandler {
	documents: Arc<DocumentStateManager>,
}

fn base_range_to_lsp(range: xeno_primitives::lsp::LspRange) -> lsp_types::Range {
	lsp_types::Range {
		start: lsp_types::Position {
			line: range.start.line,
			character: range.start.character,
		},
		end: lsp_types::Position {
			line: range.end.line,
			character: range.end.character,
		},
	}
}

impl DocumentSyncEventHandler {
	/// Create a new event handler.
	pub fn new(documents: Arc<DocumentStateManager>) -> Self {
		Self { documents }
	}
}

impl LspEventHandler for DocumentSyncEventHandler {
	fn on_diagnostics(
		&self,
		_server_id: LanguageServerId,
		uri: Uri,
		diagnostics: Vec<Diagnostic>,
		version: Option<i32>,
	) {
		self.documents
			.update_diagnostics(&uri, diagnostics, version);
	}

	fn on_progress(&self, server_id: LanguageServerId, params: lsp_types::ProgressParams) {
		self.documents.update_progress(server_id, params);
	}
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
	/// Create a new document sync coordinator with a pre-configured registry.
	///
	/// Use this when you need to set up the event handler before creating the sync.
	/// The registry should be created with [`Registry::with_event_handler`] using
	/// a [`DocumentSyncEventHandler`].
	///
	/// For a simpler setup that wires everything together, use [`create`](Self::create).
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
		self.open_document_text(path, language, text.to_string())
			.await
	}

	/// Open a document using an owned snapshot.
	pub async fn open_document_text(
		&self,
		path: &Path,
		language: &str,
		text: String,
	) -> Result<ClientHandle> {
		let client = self.registry.get_or_start(language, path)?;

		let uri = self
			.documents
			.register(path, Some(language))
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		let version = self.documents.get_version(&uri).unwrap_or(0);

		client.text_document_did_open(uri.clone(), language.to_string(), version, text)?;

		self.documents.mark_opened(&uri, version);

		Ok(client)
	}

	/// Notify language servers of a document change.
	///
	/// This sends a full document sync (the entire content). For incremental
	/// sync, use [`notify_change_incremental_no_content`](Self::notify_change_incremental_no_content).
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	/// * `language` - Language ID
	/// * `text` - New document content
	pub async fn notify_change_full(&self, path: &Path, language: &str, text: &Rope) -> Result<()> {
		self.notify_change_full_text(path, language, text.to_string())
			.await
	}

	/// Notify language servers of a full document change using an owned snapshot.
	pub async fn notify_change_full_text(
		&self,
		path: &Path,
		language: &str,
		text: String,
	) -> Result<()> {
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			self.open_document_text(path, language, text).await?;
			return Ok(());
		}

		let Some(client) = self.registry.get(language, path) else {
			self.open_document_text(path, language, text).await?;
			return Ok(());
		};

		if !client.is_initialized() {
			return Err(crate::Error::NotReady);
		}

		let version = self
			.documents
			.queue_change(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		if let Err(err) = client.text_document_did_change_full(uri.clone(), version, text) {
			self.documents.mark_force_full_sync(&uri);
			return Err(err);
		}
		self.documents.ack_change(&uri, version);

		Ok(())
	}

	/// Notify language servers of a document change with an ack after write.
	pub async fn notify_change_full_with_ack(
		&self,
		path: &Path,
		language: &str,
		text: &Rope,
	) -> Result<Option<oneshot::Receiver<()>>> {
		self.notify_change_full_with_ack_text(path, language, text.to_string())
			.await
	}

	/// Notify language servers of a full document change with an ack and owned snapshot.
	pub async fn notify_change_full_with_ack_text(
		&self,
		path: &Path,
		language: &str,
		text: String,
	) -> Result<Option<oneshot::Receiver<()>>> {
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			self.open_document_text(path, language, text).await?;
			return Ok(None);
		}

		let Some(client) = self.registry.get(language, path) else {
			self.open_document_text(path, language, text).await?;
			return Ok(None);
		};

		if !client.is_initialized() {
			return Err(crate::Error::NotReady);
		}

		let version = self
			.documents
			.queue_change(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		let ack = match client.text_document_did_change_full_with_ack(uri.clone(), version, text) {
			Ok(ack) => ack,
			Err(err) => {
				self.documents.mark_force_full_sync(&uri);
				return Err(err);
			}
		};
		Ok(Some(self.wrap_ack(uri, version, ack)))
	}

	/// Notify language servers of an incremental document change without content.
	///
	/// This variant does not require the full document content, making it suitable
	/// for debounced incremental sync where we know the document is already open.
	/// If the document is not open, returns an error (caller should fall back to
	/// full sync or re-open the document).
	pub async fn notify_change_incremental_no_content(
		&self,
		path: &Path,
		language: &str,
		changes: Vec<LspDocumentChange>,
	) -> Result<()> {
		if changes.is_empty() {
			return Ok(());
		}

		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			return Err(crate::Error::Protocol(
				"Document not opened for incremental sync".into(),
			));
		}

		let Some(client) = self.registry.get(language, path) else {
			return Err(crate::Error::Protocol("No client for language".into()));
		};

		if !client.is_initialized() {
			return Err(crate::Error::NotReady);
		}

		let content_changes: Vec<TextDocumentContentChangeEvent> = changes
			.into_iter()
			.map(|change| TextDocumentContentChangeEvent {
				range: Some(base_range_to_lsp(change.range)),
				range_length: None,
				text: change.new_text,
			})
			.collect();

		let version = self
			.documents
			.queue_change(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		if let Err(err) = client.text_document_did_change(uri.clone(), version, content_changes) {
			self.documents.mark_force_full_sync(&uri);
			return Err(err);
		}
		self.documents.ack_change(&uri, version);

		Ok(())
	}

	/// Like [`notify_change_incremental_no_content`] but returns an ack receiver.
	pub async fn notify_change_incremental_no_content_with_ack(
		&self,
		path: &Path,
		language: &str,
		changes: Vec<LspDocumentChange>,
	) -> Result<Option<oneshot::Receiver<()>>> {
		if changes.is_empty() {
			return Ok(None);
		}

		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			return Err(crate::Error::Protocol(
				"Document not opened for incremental sync".into(),
			));
		}

		let Some(client) = self.registry.get(language, path) else {
			return Err(crate::Error::Protocol("No client for language".into()));
		};

		if !client.is_initialized() {
			return Err(crate::Error::NotReady);
		}

		let content_changes: Vec<TextDocumentContentChangeEvent> = changes
			.into_iter()
			.map(|change| TextDocumentContentChangeEvent {
				range: Some(base_range_to_lsp(change.range)),
				range_length: None,
				text: change.new_text,
			})
			.collect();

		let version = self
			.documents
			.queue_change(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		let ack =
			match client.text_document_did_change_with_ack(uri.clone(), version, content_changes) {
				Ok(ack) => ack,
				Err(err) => {
					self.documents.mark_force_full_sync(&uri);
					return Err(err);
				}
			};
		Ok(Some(self.wrap_ack(uri, version, ack)))
	}

	fn wrap_ack(
		&self,
		uri: Uri,
		version: i32,
		ack: oneshot::Receiver<()>,
	) -> oneshot::Receiver<()> {
		let (tx, rx) = oneshot::channel();
		let documents = self.documents.clone();
		tokio::spawn(async move {
			let _ = ack.await;
			documents.ack_change(&uri, version);
			let _ = tx.send(());
		});
		rx
	}

	/// Notify language servers that a document will be saved.
	pub fn notify_will_save(&self, path: &Path, language: &str) -> Result<()> {
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

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
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

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
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

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
		if let Some(uri) = crate::uri_from_path(path) {
			self.documents.get_diagnostics(&uri)
		} else {
			Vec::new()
		}
	}

	/// Get error count for a document.
	pub fn error_count(&self, path: &Path) -> usize {
		if let Some(uri) = crate::uri_from_path(path) {
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
		if let Some(uri) = crate::uri_from_path(path) {
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
	use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

	use super::*;

	#[test]
	fn test_document_sync_with_registry() {
		let registry = Arc::new(Registry::new());
		let documents = Arc::new(DocumentStateManager::new());
		let sync = DocumentSync::with_registry(registry, documents);

		assert_eq!(sync.total_error_count(), 0);
		assert_eq!(sync.total_warning_count(), 0);
	}

	#[test]
	fn test_document_sync_create() {
		let (sync, _registry, _documents, _receiver) = DocumentSync::create();

		assert_eq!(sync.total_error_count(), 0);
		assert_eq!(sync.total_warning_count(), 0);
	}

	#[test]
	fn test_diagnostics_event_updates_state() {
		let documents = Arc::new(DocumentStateManager::new());
		let handler = DocumentSyncEventHandler::new(documents.clone());
		let uri: Uri = "file:///test.rs".parse().expect("valid uri");

		handler.on_diagnostics(
			LanguageServerId(1),
			uri.clone(),
			vec![Diagnostic {
				range: Range::default(),
				severity: Some(DiagnosticSeverity::ERROR),
				message: "test error".to_string(),
				..Diagnostic::default()
			}],
			None,
		);

		let diagnostics = documents.get_diagnostics(&uri);
		assert_eq!(diagnostics.len(), 1);
	}
}
