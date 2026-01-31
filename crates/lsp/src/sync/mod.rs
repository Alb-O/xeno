//! Document synchronization between editor and language servers.

use std::path::Path;
use std::sync::Arc;

use lsp_types::{Diagnostic, TextDocumentContentChangeEvent, TextDocumentSaveReason, Uri};
use ropey::Rope;
use tokio::sync::oneshot;
use xeno_primitives::lsp::LspDocumentChange;

use crate::Result;
use crate::client::{ClientHandle, LanguageServerId, LspEventHandler};
use crate::document::{DiagnosticsEventReceiver, DocumentStateManager};
use crate::registry::Registry;

/// Event handler that updates [`DocumentStateManager`] with LSP events.
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
		server_id: LanguageServerId,
		uri: Uri,
		diagnostics: Vec<Diagnostic>,
		version: Option<i32>,
	) {
		tracing::debug!(
			server_id = server_id.0,
			uri = uri.as_str(),
			count = diagnostics.len(),
			version = ?version,
			"Diagnostics received by event handler"
		);
		self.documents
			.update_diagnostics(&uri, diagnostics, version);
	}

	fn on_progress(&self, server_id: LanguageServerId, params: lsp_types::ProgressParams) {
		self.documents.update_progress(server_id, params);
	}
}

/// Document synchronization coordinator.
#[derive(Clone)]
pub struct DocumentSync {
	/// Language server registry.
	registry: Arc<Registry>,
	/// Document state manager.
	documents: Arc<DocumentStateManager>,
}

impl DocumentSync {
	/// Create a new document sync coordinator with a pre-configured registry.
	pub fn with_registry(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		Self {
			registry,
			documents,
		}
	}

	/// Create a document sync coordinator and a properly configured registry.
	pub fn create(
		transport: Arc<dyn crate::client::transport::LspTransport>,
	) -> (
		Self,
		Arc<Registry>,
		Arc<DocumentStateManager>,
		DiagnosticsEventReceiver,
	) {
		let (documents, event_receiver) = DocumentStateManager::with_events();
		let documents = Arc::new(documents);
		let registry = Arc::new(Registry::new(transport));

		let sync = Self {
			registry: registry.clone(),
			documents: documents.clone(),
		};

		(sync, registry, documents, event_receiver)
	}

	/// Open a document with the appropriate language server.
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
		let client = self.registry.get_or_start(language, path).await?;

		let uri = self
			.documents
			.register(path, Some(language))
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		let version = self.documents.get_version(&uri).unwrap_or(0);

		client
			.text_document_did_open(uri.clone(), language.to_string(), version, text)
			.await?;

		self.documents.mark_opened(&uri, version);

		Ok(client)
	}

	/// Notify language servers of a document change.
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

		if let Err(err) = client
			.text_document_did_change_full(uri.clone(), version, text)
			.await
		{
			self.documents.mark_force_full_sync(&uri);
			return Err(err);
		}
		self.documents.ack_change(&uri, version);

		Ok(())
	}

	/// Notify language servers of a document change with a barrier after write.
	pub async fn notify_change_full_with_barrier(
		&self,
		path: &Path,
		language: &str,
		text: &Rope,
	) -> Result<Option<oneshot::Receiver<()>>> {
		self.notify_change_full_with_barrier_text(path, language, text.to_string())
			.await
	}

	/// Notify language servers of a full document change with a barrier and owned snapshot.
	pub async fn notify_change_full_with_barrier_text(
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

		let barrier = match client
			.text_document_did_change_full_with_barrier(uri.clone(), version, text)
			.await
		{
			Ok(barrier) => barrier,
			Err(err) => {
				self.documents.mark_force_full_sync(&uri);
				return Err(err);
			}
		};
		Ok(Some(self.wrap_barrier(uri, version, barrier)))
	}

	/// Notify language servers of an incremental document change without content.
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

		if let Err(err) = client
			.text_document_did_change(uri.clone(), version, content_changes)
			.await
		{
			self.documents.mark_force_full_sync(&uri);
			return Err(err);
		}
		self.documents.ack_change(&uri, version);

		Ok(())
	}

	/// Like [`notify_change_incremental_no_content`] but returns a barrier receiver.
	pub async fn notify_change_incremental_no_content_with_barrier(
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

		let barrier = match client
			.text_document_did_change_with_barrier(uri.clone(), version, content_changes)
			.await
		{
			Ok(barrier) => barrier,
			Err(err) => {
				self.documents.mark_force_full_sync(&uri);
				return Err(err);
			}
		};
		Ok(Some(self.wrap_barrier(uri, version, barrier)))
	}

	fn wrap_barrier(
		&self,
		uri: Uri,
		version: i32,
		barrier: oneshot::Receiver<()>,
	) -> oneshot::Receiver<()> {
		let (tx, rx) = oneshot::channel();
		let documents = self.documents.clone();
		tokio::spawn(async move {
			let _ = barrier.await;
			documents.ack_change(&uri, version);
			let _ = tx.send(());
		});
		rx
	}

	/// Notify language servers that a document will be saved.
	pub async fn notify_will_save(&self, path: &Path, language: &str) -> Result<()> {
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if let Some(client) = self.registry.get(language, path) {
			client
				.text_document_will_save(uri, TextDocumentSaveReason::MANUAL)
				.await?;
		}

		Ok(())
	}

	/// Notify language servers that a document was saved.
	pub async fn notify_did_save(
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
			client.text_document_did_save(uri, text_content).await?;
		}

		Ok(())
	}

	/// Close a document with language servers.
	pub async fn close_document(&self, path: &Path, language: &str) -> Result<()> {
		let uri = crate::uri_from_path(path)
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if self.documents.is_opened(&uri)
			&& let Some(client) = self.registry.get(language, path)
		{
			client.text_document_did_close(uri.clone()).await?;
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

	/// Get an Arc to the document state manager.
	pub fn documents_arc(&self) -> Arc<DocumentStateManager> {
		self.documents.clone()
	}
}

#[cfg(test)]
mod tests;
