//! LSP integration extension for Xeno.
//!
//! Manages language server lifecycle and document synchronization via hooks for
//! buffer events (open, change, close). [`LspManager`] wraps the LSP [`Registry`]
//! and is stored in [`ExtensionMap`] as `Arc<LspManager>`.
//!
//! The `Arc<Registry>` is also stored separately in the ExtensionMap for direct
//! access by rendering code that needs diagnostics.

mod hooks;

use std::path::Path;
use std::sync::Arc;

use linkme::distributed_slice;
use tracing::{debug, warn};
use xeno_api::editor::extensions::{EXTENSIONS, ExtensionInitDef, ExtensionMap};
use xeno_lsp::lsp_types::Url;
use xeno_lsp::{LanguageServerConfig, Registry};

/// Manager for LSP client connections.
///
/// Wraps the LSP [`Registry`] and provides high-level methods for document
/// synchronization. This is stored in [`ExtensionMap`] as `Arc<LspManager>`.
///
/// The underlying `Arc<Registry>` is also stored in the ExtensionMap for direct
/// access by rendering code.
pub struct LspManager {
	registry: Arc<Registry>,
}

impl Default for LspManager {
	fn default() -> Self {
		Self::new()
	}
}

impl LspManager {
	/// Create a new LSP manager with an empty registry.
	pub fn new() -> Self {
		Self {
			registry: Arc::new(Registry::new()),
		}
	}

	/// Register a language server configuration.
	pub fn register(&self, language: impl Into<String>, config: LanguageServerConfig) {
		self.registry.register(language, config);
	}

	/// Get the underlying registry for direct access.
	pub fn registry(&self) -> &Registry {
		&self.registry
	}

	/// Get a clone of the Arc<Registry> for sharing.
	pub fn registry_arc(&self) -> Arc<Registry> {
		Arc::clone(&self.registry)
	}

	/// Notify language servers that a document was opened.
	///
	/// Starts a language server if one isn't running for the file's language and project root.
	pub async fn did_open(
		&self,
		path: &Path,
		text: &str,
		language: Option<&str>,
		version: u64,
	) -> Option<()> {
		eprintln!("DEBUG LspManager::did_open: path={:?}, language={:?}", path, language);
		let language = language?;
		let uri = Url::from_file_path(path).ok()?;

		eprintln!("DEBUG LspManager::did_open: calling get_or_start for language={}", language);
		let client = match self.registry.get_or_start(language, path).await {
			Ok(client) => {
				eprintln!("DEBUG LspManager::did_open: got client");
				client
			}
			Err(e) => {
				eprintln!("DEBUG LspManager::did_open: get_or_start error: {}", e);
				debug!(language = language, error = %e, "LSP: No server available");
				return None;
			}
		};

		eprintln!("DEBUG LspManager::did_open: waiting for client initialization");
		client.wait_initialized().await;
		eprintln!("DEBUG LspManager::did_open: client initialized");

		if let Err(e) =
			client.text_document_did_open(uri, language.to_string(), version as i32, text.into())
		{
			eprintln!("DEBUG LspManager::did_open: didOpen failed: {}", e);
			warn!(error = %e, "LSP: didOpen failed");
		}
		eprintln!("DEBUG LspManager::did_open: success");

		Some(())
	}

	/// Notify language servers that a document changed.
	pub async fn did_change(
		&self,
		path: &Path,
		text: &str,
		language: Option<&str>,
		version: u64,
	) -> Option<()> {
		let language = language?;
		let uri = Url::from_file_path(path).ok()?;
		let root_path = path.parent()?;
		let client = self.registry.get(language, root_path)?;

		if !client.is_initialized() {
			return None;
		}

		if let Err(e) = client.text_document_did_change_full(uri, version as i32, text.into()) {
			warn!(error = %e, "LSP: didChange failed");
		}

		Some(())
	}

	/// Notify language servers that a document was closed.
	pub async fn did_close(&self, path: &Path, language: Option<&str>) -> Option<()> {
		let language = language?;
		let uri = Url::from_file_path(path).ok()?;

		let root_path = path.parent()?;
		let client = self.registry.get(language, root_path)?;

		if !client.is_initialized() {
			return None;
		}

		if let Err(e) = client.text_document_did_close(uri) {
			warn!(error = %e, "LSP: didClose failed");
		}

		Some(())
	}

	/// Shutdown all language servers.
	pub async fn shutdown_all(&self) {
		self.registry.shutdown_all().await;
	}

	/// Get diagnostics for a file path from the appropriate language server.
	pub fn get_diagnostics(&self, path: &Path, language: Option<&str>) -> Vec<xeno_lsp::lsp_types::Diagnostic> {
		let Some(language) = language else {
			return Vec::new();
		};
		let Some(root_path) = path.parent() else {
			return Vec::new();
		};
		let Some(client) = self.registry.get(language, root_path) else {
			return Vec::new();
		};
		let Ok(uri) = Url::from_file_path(path) else {
			return Vec::new();
		};
		client.diagnostics(&uri)
	}

	/// Get the total diagnostic revision across all active servers.
	///
	/// This counter increases each time any server publishes diagnostics.
	/// Can be used to detect when diagnostics have changed and a redraw is needed.
	pub fn diagnostic_revision(&self) -> u64 {
		self.registry.total_diagnostic_revision()
	}
}

fn init_lsp(map: &mut ExtensionMap) {
	let manager = Arc::new(LspManager::new());

	manager.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			root_markers: vec!["Cargo.toml".into(), "rust-project.json".into()],
			..Default::default()
		},
	);

	manager.register(
		"typescript",
		LanguageServerConfig {
			command: "typescript-language-server".into(),
			args: vec!["--stdio".into()],
			root_markers: vec!["tsconfig.json".into(), "package.json".into()],
			..Default::default()
		},
	);

	manager.register(
		"javascript",
		LanguageServerConfig {
			command: "typescript-language-server".into(),
			args: vec!["--stdio".into()],
			root_markers: vec!["package.json".into(), "jsconfig.json".into()],
			..Default::default()
		},
	);

	manager.register(
		"python",
		LanguageServerConfig {
			command: "pylsp".into(),
			root_markers: vec![
				"pyproject.toml".into(),
				"setup.py".into(),
				"requirements.txt".into(),
			],
			..Default::default()
		},
	);

	manager.register(
		"go",
		LanguageServerConfig {
			command: "gopls".into(),
			root_markers: vec!["go.mod".into(), "go.work".into()],
			..Default::default()
		},
	);

	// Insert the registry Arc for direct access by rendering code
	map.insert(manager.registry_arc());
	// Insert the manager for hook-based document synchronization
	map.insert(manager);
}

#[distributed_slice(EXTENSIONS)]
static LSP_INIT: ExtensionInitDef = ExtensionInitDef {
	id: "lsp",
	priority: 50,
	init: init_lsp,
};
