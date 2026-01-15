//! LSP integration for the xeno editor.
//!
//! This module bridges the editor's buffer system with LSP functionality,
//! providing document synchronization, diagnostics, and language features.
//!
//! # Feature Flag
//!
//! This module is only available when the `lsp` feature is enabled:
//!
//! ```toml
//! [dependencies]
//! xeno-api = { version = "0.1", features = ["lsp"] }
//! ```
//!
//! # Architecture
//!
//! The LSP integration consists of:
//!
//! - [`LspManager`] - Central coordinator for LSP functionality
//! - Document synchronization via [`xeno_lsp::DocumentSync`]
//! - Server registry via [`xeno_lsp::Registry`]
//!
//! # Usage
//!
//! ```ignore
//! use xeno_api::lsp::LspManager;
//!
//! let lsp = LspManager::new();
//!
//! // Configure language servers
//! lsp.configure_server("rust", LanguageServerConfig {
//!     command: "rust-analyzer".into(),
//!     root_markers: vec!["Cargo.toml".into()],
//!     ..Default::default()
//! });
//!
//! // Open a document
//! lsp.on_buffer_open(&buffer).await?;
//!
//! // Notify of changes
//! lsp.on_buffer_change(&buffer).await?;
//! ```

use std::path::Path;
use std::sync::Arc;

// Re-export for consumers
pub use xeno_lsp::DiagnosticsEvent as LspDiagnosticsEvent;
// Re-export types needed by consumers
pub use xeno_lsp::LanguageServerConfig;
use xeno_lsp::lsp_types::{TextDocumentSyncCapability, TextDocumentSyncKind};
use xeno_lsp::{
	ClientHandle, DiagnosticsEvent, DiagnosticsEventReceiver, DocumentStateManager, DocumentSync,
	OffsetEncoding, Registry, Result,
};
use xeno_primitives::LspDocumentChange;

use crate::buffer::Buffer;

/// Central manager for LSP functionality.
///
/// Coordinates language server lifecycle, document synchronization,
/// and provides access to language features.
pub struct LspManager {
	/// Document synchronization coordinator.
	sync: DocumentSync,
	/// Receiver for diagnostic update events.
	diagnostics_receiver: Option<DiagnosticsEventReceiver>,
}

impl LspManager {
	/// Create a new LSP manager.
	///
	/// This sets up the event handler so diagnostics and other LSP events
	/// are properly routed to the document state manager.
	pub fn new() -> Self {
		let (sync, _registry, _documents, diagnostics_receiver) = DocumentSync::create();
		Self {
			sync,
			diagnostics_receiver: Some(diagnostics_receiver),
		}
	}

	/// Create an LSP manager with existing registry and document state.
	///
	/// Note: The registry should be created with [`Registry::with_event_handler`]
	/// using a [`DocumentSyncEventHandler`] to ensure diagnostics are properly routed.
	/// This constructor does not provide diagnostic events.
	pub fn with_state(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		let sync = DocumentSync::with_registry(registry, documents);
		Self {
			sync,
			diagnostics_receiver: None,
		}
	}

	/// Poll for pending diagnostic events.
	///
	/// Returns any diagnostic update events that have occurred since the last poll.
	/// This should be called during the editor's main loop to process LSP events.
	pub fn poll_diagnostics(&mut self) -> Vec<DiagnosticsEvent> {
		let Some(ref mut receiver) = self.diagnostics_receiver else {
			return Vec::new();
		};

		let mut events = Vec::new();
		while let Ok(event) = receiver.try_recv() {
			events.push(event);
		}
		events
	}

	/// Get the diagnostics version counter.
	///
	/// This counter increments every time any document's diagnostics change.
	/// Useful for detecting if a re-render is needed without polling events.
	pub fn diagnostics_version(&self) -> u64 {
		self.sync.documents().diagnostics_version()
	}

	/// Configure a language server.
	pub fn configure_server(&self, language: impl Into<String>, config: LanguageServerConfig) {
		self.sync.registry().register(language, config);
	}

	/// Remove a language server configuration.
	pub fn remove_server(&self, language: &str) {
		self.sync.registry().unregister(language);
	}

	/// Get the document sync coordinator.
	pub fn sync(&self) -> &DocumentSync {
		&self.sync
	}

	/// Get the server registry.
	pub fn registry(&self) -> &Registry {
		self.sync.registry()
	}

	/// Get the document state manager.
	pub fn documents(&self) -> &DocumentStateManager {
		self.sync.documents()
	}

	/// Called when a buffer is opened.
	///
	/// Starts the appropriate language server and opens the document.
	pub async fn on_buffer_open(&self, buffer: &Buffer) -> Result<Option<ClientHandle>> {
		let Some(path) = buffer.path() else {
			return Ok(None);
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(None);
		};

		if self.sync.registry().get_config(language).is_none() {
			return Ok(None);
		}

		// Canonicalize path to absolute (required for LSP URIs)
		let abs_path = path
			.canonicalize()
			.unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(&path));

		let content = buffer.doc().content.clone();
		let client = self
			.sync
			.open_document(&abs_path, language, &content)
			.await?;
		Ok(Some(client))
	}

	/// Called when a buffer's content changes.
	///
	/// Sends a full document sync to the language server.
	pub async fn on_buffer_change(&self, buffer: &Buffer) -> Result<()> {
		let Some(path) = &buffer.path() else {
			return Ok(());
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(());
		};

		let content = buffer.doc().content.clone();
		self.sync.notify_change_full(path, language, &content).await
	}

	/// Called when a buffer's content changes incrementally.
	///
	/// Sends incremental document sync to the language server using pre-computed ranges.
	pub async fn on_buffer_change_incremental(
		&self,
		buffer: &Buffer,
		changes: Vec<LspDocumentChange>,
	) -> Result<()> {
		let Some(path) = &buffer.path() else {
			return Ok(());
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(());
		};

		let content = buffer.doc().content.clone();
		self.sync
			.notify_change_incremental(path, language, &content, changes)
			.await
	}

	/// Called before a buffer is saved.
	pub fn on_buffer_will_save(&self, buffer: &Buffer) -> Result<()> {
		let Some(path) = &buffer.path() else {
			return Ok(());
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(());
		};

		self.sync.notify_will_save(path, language)
	}

	/// Called after a buffer is saved.
	pub fn on_buffer_did_save(&self, buffer: &Buffer, include_text: bool) -> Result<()> {
		let Some(path) = &buffer.path() else {
			return Ok(());
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(());
		};

		let doc = buffer.doc();
		let text = if include_text {
			Some(&doc.content)
		} else {
			None
		};
		self.sync
			.notify_did_save(path, language, include_text, text)
	}

	/// Called when a buffer is closed.
	pub fn on_buffer_close(&self, buffer: &Buffer) -> Result<()> {
		let Some(path) = &buffer.path() else {
			return Ok(());
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(());
		};

		self.sync.close_document(path, language)
	}

	/// Get diagnostics for a buffer.
	pub fn get_diagnostics(&self, buffer: &Buffer) -> Vec<xeno_lsp::lsp_types::Diagnostic> {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync.get_diagnostics(p))
			.unwrap_or_default()
	}

	/// Get error count for a buffer.
	pub fn error_count(&self, buffer: &Buffer) -> usize {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync.error_count(p))
			.unwrap_or(0)
	}

	/// Get warning count for a buffer.
	pub fn warning_count(&self, buffer: &Buffer) -> usize {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync.warning_count(p))
			.unwrap_or(0)
	}

	/// Get total error count across all documents.
	pub fn total_error_count(&self) -> usize {
		self.sync.total_error_count()
	}

	/// Get total warning count across all documents.
	pub fn total_warning_count(&self) -> usize {
		self.sync.total_warning_count()
	}

	/// Prepare a position-based LSP request. Returns None if no client available.
	pub(crate) fn prepare_position_request(
		&self,
		buffer: &Buffer,
	) -> Result<
		Option<(
			ClientHandle,
			xeno_lsp::lsp_types::Uri,
			xeno_lsp::lsp_types::Position,
		)>,
	> {
		let Some(path) = buffer.path() else {
			return Ok(None);
		};
		let Some(language) = buffer.file_type() else {
			return Ok(None);
		};

		let abs_path = path
			.canonicalize()
			.unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(&path));

		let Some(client) = self.sync.registry().get(&language, &abs_path) else {
			return Ok(None);
		};

		let uri = xeno_lsp::uri_from_path(&abs_path)
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = client.offset_encoding();
		let position =
			xeno_lsp::char_to_lsp_position(&buffer.doc().content, buffer.cursor, encoding)
				.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		Ok(Some((client, uri, position)))
	}

	/// Request hover information at the cursor position.
	pub async fn hover(&self, buffer: &Buffer) -> Result<Option<xeno_lsp::lsp_types::Hover>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.hover(uri, position).await
	}

	/// Request completions at the cursor position.
	pub async fn completion(
		&self,
		buffer: &Buffer,
	) -> Result<Option<xeno_lsp::lsp_types::CompletionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.completion(uri, position, None).await
	}

	/// Request go to definition at the cursor position.
	pub async fn goto_definition(
		&self,
		buffer: &Buffer,
	) -> Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.goto_definition(uri, position).await
	}

	/// Request references at the cursor position.
	pub async fn references(
		&self,
		buffer: &Buffer,
		include_declaration: bool,
	) -> Result<Option<Vec<xeno_lsp::lsp_types::Location>>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.references(uri, position, include_declaration).await
	}

	/// Request formatting for the entire document.
	pub async fn format(
		&self,
		buffer: &Buffer,
	) -> Result<Option<Vec<xeno_lsp::lsp_types::TextEdit>>> {
		let Some((client, uri, _)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		let options = xeno_lsp::lsp_types::FormattingOptions {
			tab_size: 4,
			insert_spaces: false,
			..Default::default()
		};
		client.formatting(uri, options).await
	}

	/// Shutdown all language servers.
	pub async fn shutdown_all(&self) {
		self.sync.registry().shutdown_all().await;
	}

	/// Returns the server encoding if incremental sync is supported.
	pub fn incremental_encoding_for_buffer(&self, buffer: &Buffer) -> Option<OffsetEncoding> {
		let path = buffer.path()?;
		let language = buffer.file_type()?;
		self.incremental_encoding(&path, &language)
	}

	/// Returns the server encoding for a buffer, defaulting to UTF-16.
	pub fn offset_encoding_for_buffer(&self, buffer: &Buffer) -> OffsetEncoding {
		let Some(path) = buffer.path() else {
			return OffsetEncoding::Utf16;
		};
		let Some(language) = buffer.file_type() else {
			return OffsetEncoding::Utf16;
		};

		self.sync
			.registry()
			.get(&language, &path)
			.map(|client| client.offset_encoding())
			.unwrap_or(OffsetEncoding::Utf16)
	}

	fn incremental_encoding(&self, path: &Path, language: &str) -> Option<OffsetEncoding> {
		let client = self.sync.registry().get(language, path)?;
		let caps = client.try_capabilities()?;
		let supports_incremental = match &caps.text_document_sync {
			Some(TextDocumentSyncCapability::Kind(kind)) => {
				*kind == TextDocumentSyncKind::INCREMENTAL
			}
			Some(TextDocumentSyncCapability::Options(options)) => {
				matches!(options.change, Some(TextDocumentSyncKind::INCREMENTAL))
			}
			None => false,
		};

		if supports_incremental {
			Some(client.offset_encoding())
		} else {
			None
		}
	}
}

impl Default for LspManager {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_lsp_manager_creation() {
		let manager = LspManager::new();
		assert_eq!(manager.total_error_count(), 0);
		assert_eq!(manager.total_warning_count(), 0);
	}

	#[test]
	fn test_configure_server() {
		let manager = LspManager::new();
		manager.configure_server(
			"rust",
			LanguageServerConfig {
				command: "rust-analyzer".into(),
				root_markers: vec!["Cargo.toml".into()],
				..Default::default()
			},
		);

		assert!(manager.registry().get_config("rust").is_some());
		assert!(manager.registry().get_config("python").is_none());
	}
}
