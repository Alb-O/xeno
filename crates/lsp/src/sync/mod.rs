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

/// Barrier behavior for document change dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierMode {
	/// Send and ack synchronously without waiting for a write barrier.
	None,
	/// Send with write barrier and ack when the barrier resolves.
	Tracked,
}

/// Change payload sent through [`DocumentSync::send_change`].
#[derive(Debug, Clone)]
pub enum ChangePayload {
	/// Full document content replacement.
	FullText(String),
	/// Incremental edits without full content snapshots.
	Incremental(Vec<LspDocumentChange>),
}

/// Unified document change request.
#[derive(Debug)]
pub struct ChangeRequest<'a> {
	/// Filesystem path of the target document.
	pub path: &'a Path,
	/// Language identifier for server lookup/open.
	pub language: &'a str,
	/// Payload to send.
	pub payload: ChangePayload,
	/// Barrier mode for this change.
	pub barrier: BarrierMode,
	/// Whether full-text payloads may open/reopen a missing document/client.
	pub open_if_needed: bool,
}

impl<'a> ChangeRequest<'a> {
	/// Construct a full-text change request.
	pub fn full_text(path: &'a Path, language: &'a str, text: String) -> Self {
		Self {
			path,
			language,
			payload: ChangePayload::FullText(text),
			barrier: BarrierMode::None,
			open_if_needed: true,
		}
	}

	/// Construct an incremental change request.
	pub fn incremental(path: &'a Path, language: &'a str, changes: Vec<LspDocumentChange>) -> Self {
		Self {
			path,
			language,
			payload: ChangePayload::Incremental(changes),
			barrier: BarrierMode::None,
			open_if_needed: false,
		}
	}

	/// Set barrier behavior.
	pub fn with_barrier(mut self, mode: BarrierMode) -> Self {
		self.barrier = mode;
		self
	}

	/// Configure open-if-needed behavior.
	pub fn with_open_if_needed(mut self, open_if_needed: bool) -> Self {
		self.open_if_needed = open_if_needed;
		self
	}
}

/// Outcome of a unified change dispatch.
pub struct ChangeDispatch {
	/// Completion signal for tracked barriers.
	pub barrier: Option<oneshot::Receiver<()>>,
	/// Version queued for this change when one was sent.
	pub applied_version: Option<i32>,
	/// Whether this request opened/reopened the document instead of sending a didChange.
	pub opened_document: bool,
}

impl std::fmt::Debug for ChangeDispatch {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ChangeDispatch")
			.field("has_barrier", &self.barrier.is_some())
			.field("applied_version", &self.applied_version)
			.field("opened_document", &self.opened_document)
			.finish()
	}
}

impl ChangeDispatch {
	fn noop() -> Self {
		Self {
			barrier: None,
			applied_version: None,
			opened_document: false,
		}
	}

	fn opened() -> Self {
		Self {
			barrier: None,
			applied_version: None,
			opened_document: true,
		}
	}
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
	fn on_diagnostics(&self, server_id: LanguageServerId, uri: Uri, diagnostics: Vec<Diagnostic>, version: Option<i32>) {
		tracing::debug!(
			server_id = %server_id,
			uri = uri.as_str(),
			count = diagnostics.len(),
			version = ?version,
			"Diagnostics received by event handler"
		);
		self.documents.update_diagnostics(&uri, diagnostics, version);
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
	/// Shared worker runtime for async helper tasks.
	worker_runtime: xeno_worker::WorkerRuntime,
}

impl DocumentSync {
	/// Create a new document sync coordinator with a pre-configured registry.
	pub fn with_registry(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		Self {
			worker_runtime: registry.worker_runtime(),
			registry,
			documents,
		}
	}

	/// Create a document sync coordinator and a properly configured registry.
	pub fn create(
		transport: Arc<dyn crate::client::transport::LspTransport>,
		worker_runtime: xeno_worker::WorkerRuntime,
	) -> (Self, Arc<Registry>, Arc<DocumentStateManager>, DiagnosticsEventReceiver) {
		let (documents, event_receiver) = DocumentStateManager::with_events();
		let documents = Arc::new(documents);
		let registry = Arc::new(Registry::new(transport, worker_runtime.clone()));

		let sync = Self {
			registry: registry.clone(),
			documents: documents.clone(),
			worker_runtime,
		};

		(sync, registry, documents, event_receiver)
	}

	/// Open a document with the appropriate language server.
	pub async fn open_document(&self, path: &Path, language: &str, text: &Rope) -> Result<ClientHandle> {
		self.ensure_open_text(path, language, text.to_string()).await
	}

	/// Open a document using an owned snapshot.
	pub async fn ensure_open_text(&self, path: &Path, language: &str, text: String) -> Result<ClientHandle> {
		let acquired = self.registry.acquire(language, path).await?;
		let client = acquired.handle;

		let uri = self
			.documents
			.register(path, Some(language))
			.ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		let version = self.documents.get_version(&uri).unwrap_or(0);

		client.text_document_did_open(uri.clone(), language.to_string(), version, text).await?;

		self.documents.mark_opened(&uri, version);

		Ok(client)
	}

	/// Send a unified document change request.
	pub async fn send_change(&self, request: ChangeRequest<'_>) -> Result<ChangeDispatch> {
		let ChangeRequest {
			path,
			language,
			payload,
			barrier,
			open_if_needed,
		} = request;

		if let ChangePayload::Incremental(changes) = &payload
			&& changes.is_empty()
		{
			return Ok(ChangeDispatch::noop());
		}

		let uri = crate::uri_from_path(path).ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if !self.documents.is_opened(&uri) {
			return match payload {
				ChangePayload::FullText(text) if open_if_needed => {
					self.ensure_open_text(path, language, text).await?;
					Ok(ChangeDispatch::opened())
				}
				ChangePayload::FullText(_) => Err(crate::Error::Protocol("Document not opened for full sync".into())),
				ChangePayload::Incremental(_) => Err(crate::Error::Protocol("Document not opened for incremental sync".into())),
			};
		}

		let Some(client) = self.registry.get(language, path) else {
			return match payload {
				ChangePayload::FullText(text) if open_if_needed => {
					self.ensure_open_text(path, language, text).await?;
					Ok(ChangeDispatch::opened())
				}
				ChangePayload::FullText(_) => Err(crate::Error::Protocol("No client for language".into())),
				ChangePayload::Incremental(_) => Err(crate::Error::Protocol("No client for language".into())),
			};
		};

		if !client.is_initialized() {
			return Err(crate::Error::NotReady);
		}

		let version = self
			.documents
			.queue_change(&uri)
			.ok_or_else(|| crate::Error::Protocol("Document not registered".into()))?;

		match payload {
			ChangePayload::FullText(text) => self.dispatch_full_change(client, uri, version, text, barrier).await,
			ChangePayload::Incremental(changes) => self.dispatch_incremental_change(client, uri, version, changes, barrier).await,
		}
	}

	async fn dispatch_full_change(&self, client: ClientHandle, uri: Uri, version: i32, text: String, barrier: BarrierMode) -> Result<ChangeDispatch> {
		match barrier {
			BarrierMode::None => {
				if let Err(err) = client.text_document_did_change_full(uri.clone(), version, text).await {
					self.documents.mark_force_full_sync(&uri);
					return Err(err);
				}
				if !self.documents.ack_change(&uri, version) {
					tracing::warn!(uri = uri.as_str(), version, "LSP immediate ack mismatch");
				}
				Ok(ChangeDispatch {
					barrier: None,
					applied_version: Some(version),
					opened_document: false,
				})
			}
			BarrierMode::Tracked => {
				let barrier = match client.text_document_did_change_full_with_barrier(uri.clone(), version, text).await {
					Ok(barrier) => barrier,
					Err(err) => {
						self.documents.mark_force_full_sync(&uri);
						return Err(err);
					}
				};
				Ok(ChangeDispatch {
					barrier: Some(self.wrap_barrier(uri, version, barrier)),
					applied_version: Some(version),
					opened_document: false,
				})
			}
		}
	}

	async fn dispatch_incremental_change(
		&self,
		client: ClientHandle,
		uri: Uri,
		version: i32,
		changes: Vec<LspDocumentChange>,
		barrier: BarrierMode,
	) -> Result<ChangeDispatch> {
		let content_changes: Vec<TextDocumentContentChangeEvent> = changes
			.into_iter()
			.map(|change| TextDocumentContentChangeEvent {
				range: Some(base_range_to_lsp(change.range)),
				range_length: None,
				text: change.new_text,
			})
			.collect();

		match barrier {
			BarrierMode::None => {
				if let Err(err) = client.text_document_did_change(uri.clone(), version, content_changes).await {
					self.documents.mark_force_full_sync(&uri);
					return Err(err);
				}
				if !self.documents.ack_change(&uri, version) {
					tracing::warn!(uri = uri.as_str(), version, "LSP immediate ack mismatch");
				}
				Ok(ChangeDispatch {
					barrier: None,
					applied_version: Some(version),
					opened_document: false,
				})
			}
			BarrierMode::Tracked => {
				let barrier = match client.text_document_did_change_with_barrier(uri.clone(), version, content_changes).await {
					Ok(barrier) => barrier,
					Err(err) => {
						self.documents.mark_force_full_sync(&uri);
						return Err(err);
					}
				};
				Ok(ChangeDispatch {
					barrier: Some(self.wrap_barrier(uri, version, barrier)),
					applied_version: Some(version),
					opened_document: false,
				})
			}
		}
	}

	fn wrap_barrier(&self, uri: Uri, version: i32, barrier: oneshot::Receiver<crate::Result<()>>) -> oneshot::Receiver<()> {
		let (tx, rx) = oneshot::channel();
		let documents = self.documents.clone();
		self.worker_runtime.spawn(xeno_worker::TaskClass::Background, async move {
			match barrier.await {
				Ok(Ok(())) => {
					if !documents.ack_change(&uri, version) {
						tracing::warn!(uri = uri.as_str(), version, "LSP barrier ack mismatch");
					}
				}
				Ok(Err(e)) => {
					tracing::error!(uri = uri.as_str(), version, error = %e, "LSP write barrier failed");
					documents.mark_force_full_sync(&uri);
				}
				Err(_) => {
					tracing::error!(uri = uri.as_str(), version, "LSP barrier sender dropped");
					documents.mark_force_full_sync(&uri);
				}
			}
			let _ = tx.send(());
		});
		rx
	}

	/// Notify language servers that a document will be saved.
	pub async fn notify_will_save(&self, path: &Path, language: &str) -> Result<()> {
		let uri = crate::uri_from_path(path).ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		if let Some(client) = self.registry.get(language, path) {
			client.text_document_will_save(uri, TextDocumentSaveReason::MANUAL).await?;
		}

		Ok(())
	}

	/// Notify language servers that a document was saved.
	pub async fn notify_did_save(&self, path: &Path, language: &str, include_text: bool, text: Option<&Rope>) -> Result<()> {
		let uri = crate::uri_from_path(path).ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

		let text_content = if include_text { text.map(|t| t.to_string()) } else { None };

		if let Some(client) = self.registry.get(language, path) {
			client.text_document_did_save(uri, text_content).await?;
		}

		Ok(())
	}

	/// Close a document with language servers.
	pub async fn close_document(&self, path: &Path, language: &str) -> Result<()> {
		let uri = crate::uri_from_path(path).ok_or_else(|| crate::Error::Protocol("Invalid path".into()))?;

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
			diags.iter().filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR)).count()
		} else {
			0
		}
	}

	/// Get warning count for a document.
	pub fn warning_count(&self, path: &Path) -> usize {
		if let Some(uri) = crate::uri_from_path(path) {
			let diags = self.documents.get_diagnostics(&uri);
			diags.iter().filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::WARNING)).count()
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
