use std::sync::Arc;

use crate::{
	DiagnosticsEvent, DiagnosticsEventReceiver, DocumentStateManager, DocumentSync,
	LanguageServerConfig, Registry,
};

/// Central manager for LSP functionality.
pub struct LspManager {
	sync: DocumentSync,
	diagnostics_receiver: Option<DiagnosticsEventReceiver>,
}

impl LspManager {
	/// Create a new LSP manager.
	pub fn new() -> Self {
		let (sync, _registry, _documents, diagnostics_receiver) = DocumentSync::create();
		Self {
			sync,
			diagnostics_receiver: Some(diagnostics_receiver),
		}
	}

	/// Create an LSP manager with existing registry and document state.
	pub fn with_state(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		let sync = DocumentSync::with_registry(registry, documents);
		Self {
			sync,
			diagnostics_receiver: None,
		}
	}

	/// Poll for pending diagnostic events.
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

	/// Shutdown all language servers.
	pub async fn shutdown_all(&self) {
		self.sync.registry().shutdown_all().await;
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
		assert_eq!(manager.diagnostics_version(), 0);
	}
}
