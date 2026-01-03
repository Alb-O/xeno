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

use xeno_lsp::{
	ClientHandle, DocumentStateManager, DocumentSync, LanguageServerConfig, OffsetEncoding,
	Registry, Result,
};

use crate::buffer::Buffer;

/// Central manager for LSP functionality.
///
/// Coordinates language server lifecycle, document synchronization,
/// and provides access to language features.
pub struct LspManager {
	/// Document synchronization coordinator.
	sync: DocumentSync,
}

impl LspManager {
	/// Create a new LSP manager.
	pub fn new() -> Self {
		let registry = Arc::new(Registry::new());
		let documents = Arc::new(DocumentStateManager::new());
		let sync = DocumentSync::new(registry, documents);
		Self { sync }
	}

	/// Create an LSP manager with existing registry and document state.
	pub fn with_state(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		let sync = DocumentSync::new(registry, documents);
		Self { sync }
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
		let Some(path) = &buffer.path() else {
			return Ok(None);
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(None);
		};

		// Check if we have a server configured for this language
		if self.sync.registry().get_config(language).is_none() {
			return Ok(None);
		}

		let content = buffer.doc().content.clone();
		let client = self.sync.open_document(path, language, &content).await?;
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

	/// Called when a buffer's content changes with specific range info.
	///
	/// Sends an incremental document sync to the language server.
	pub async fn on_buffer_change_incremental(
		&self,
		buffer: &Buffer,
		start_char: usize,
		end_char: usize,
		new_text: &str,
	) -> Result<()> {
		let Some(path) = &buffer.path() else {
			return Ok(());
		};

		let Some(language) = &buffer.file_type() else {
			return Ok(());
		};

		// Get encoding from the client, default to UTF-16
		let encoding = self.get_encoding_for_path(path, language);

		let content = buffer.doc().content.clone();
		self.sync
			.notify_change_incremental(
				path, language, &content, start_char, end_char, new_text, encoding,
			)
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

	/// Get a language server client for a buffer.
	pub fn get_client(&self, buffer: &Buffer) -> Option<ClientHandle> {
		let path = buffer.path()?;
		let language = buffer.file_type()?;
		self.sync.registry().get(&language, &path)
	}

	/// Request hover information at the cursor position.
	pub async fn hover(&self, buffer: &Buffer) -> Result<Option<xeno_lsp::lsp_types::Hover>> {
		let client = match self.get_client(buffer) {
			Some(c) => c,
			None => return Ok(None),
		};

		let path = buffer.path().unwrap();
		let language = buffer.file_type().unwrap();
		let uri = xeno_lsp::lsp_types::Url::from_file_path(&path)
			.map_err(|_| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = self.get_encoding_for_path(&path, &language);
		let position =
			xeno_lsp::char_to_lsp_position(&buffer.doc().content, buffer.cursor, encoding)
				.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		client.hover(uri, position).await
	}

	/// Request completions at the cursor position.
	pub async fn completion(
		&self,
		buffer: &Buffer,
	) -> Result<Option<xeno_lsp::lsp_types::CompletionResponse>> {
		let client = match self.get_client(buffer) {
			Some(c) => c,
			None => return Ok(None),
		};

		let path = buffer.path().unwrap();
		let language = buffer.file_type().unwrap();
		let uri = xeno_lsp::lsp_types::Url::from_file_path(&path)
			.map_err(|_| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = self.get_encoding_for_path(&path, &language);
		let position =
			xeno_lsp::char_to_lsp_position(&buffer.doc().content, buffer.cursor, encoding)
				.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		client.completion(uri, position, None).await
	}

	/// Request go to definition at the cursor position.
	pub async fn goto_definition(
		&self,
		buffer: &Buffer,
	) -> Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let client = match self.get_client(buffer) {
			Some(c) => c,
			None => return Ok(None),
		};

		let path = buffer.path().unwrap();
		let language = buffer.file_type().unwrap();
		let uri = xeno_lsp::lsp_types::Url::from_file_path(&path)
			.map_err(|_| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = self.get_encoding_for_path(&path, &language);
		let position =
			xeno_lsp::char_to_lsp_position(&buffer.doc().content, buffer.cursor, encoding)
				.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		client.goto_definition(uri, position).await
	}

	/// Request references at the cursor position.
	pub async fn references(
		&self,
		buffer: &Buffer,
		include_declaration: bool,
	) -> Result<Option<Vec<xeno_lsp::lsp_types::Location>>> {
		let client = match self.get_client(buffer) {
			Some(c) => c,
			None => return Ok(None),
		};

		let path = buffer.path().unwrap();
		let language = buffer.file_type().unwrap();
		let uri = xeno_lsp::lsp_types::Url::from_file_path(&path)
			.map_err(|_| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = self.get_encoding_for_path(&path, &language);
		let position =
			xeno_lsp::char_to_lsp_position(&buffer.doc().content, buffer.cursor, encoding)
				.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		client.references(uri, position, include_declaration).await
	}

	/// Request formatting for the entire document.
	pub async fn format(
		&self,
		buffer: &Buffer,
	) -> Result<Option<Vec<xeno_lsp::lsp_types::TextEdit>>> {
		let client = match self.get_client(buffer) {
			Some(c) => c,
			None => return Ok(None),
		};

		let path = buffer.path().unwrap();
		let uri = xeno_lsp::lsp_types::Url::from_file_path(&path)
			.map_err(|_| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		// Default formatting options
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

	/// Get the offset encoding for a language server.
	fn get_encoding_for_path(&self, path: &Path, language: &str) -> OffsetEncoding {
		self.sync
			.registry()
			.get(language, path)
			.map(|c| c.offset_encoding())
			.unwrap_or_default()
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
