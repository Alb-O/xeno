//! LSP server configuration loading from the immutable registry catalog.

use std::collections::HashMap;

use serde_json::Value as JsonValue;
use thiserror::Error;

/// Errors that can occur when loading LSP configurations.
#[derive(Debug, Error)]
pub enum LspConfigError {
	#[error("failed to load lsp configuration: {0}")]
	Load(String),
}

/// Result type for LSP configuration operations.
pub type Result<T> = std::result::Result<T, LspConfigError>;

/// LSP server definition loaded from the immutable registry catalog.
#[derive(Debug, Clone)]
pub struct LspServerDef {
	/// Server identifier (e.g., "rust-analyzer", "pyright").
	pub name: String,
	/// Executable command to spawn the server.
	pub command: String,
	/// Command-line arguments passed to the server.
	pub args: Vec<String>,
	/// Environment variables set when spawning.
	pub environment: HashMap<String, String>,
	/// Server-specific configuration sent via `workspace/didChangeConfiguration`.
	pub config: Option<JsonValue>,
	/// URL for downloading/installing the server.
	pub source: Option<String>,
	/// Nix package attribute for the server binary.
	pub nix: Option<String>,
}

/// Loads LSP server configurations from the immutable registry catalog.
pub fn load_lsp_configs() -> Result<Vec<LspServerDef>> {
	Ok(xeno_registry::LSP_SERVERS
		.snapshot_guard()
		.iter_refs()
		.into_iter()
		.map(|raw| LspServerDef {
			name: raw.name_str().to_string(),
			command: raw.resolve(raw.command).to_string(),
			args: raw.args.iter().map(|&arg| raw.resolve(arg).to_string()).collect(),
			environment: raw
				.environment
				.iter()
				.map(|&(key, value)| (raw.resolve(key).to_string(), raw.resolve(value).to_string()))
				.collect(),
			config: raw.config_json.and_then(|symbol| serde_json::from_str(raw.resolve(symbol)).ok()),
			source: raw.source.map(|symbol| raw.resolve(symbol).to_string()),
			nix: raw.nix.map(|symbol| raw.resolve(symbol).to_string()),
		})
		.collect())
}

/// Language LSP configuration extracted from registry-backed language entries.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LanguageLspInfo {
	/// LSP server names for this language.
	pub servers: Vec<String>,
	/// Root markers for project detection.
	pub roots: Vec<String>,
}

/// Mapping of language name to its LSP configuration.
pub type LanguageLspMapping = HashMap<String, LanguageLspInfo>;

/// Resolved language-to-server catalog entry.
#[derive(Debug, Clone)]
pub struct ResolvedLanguageLspConfig {
	pub language: String,
	pub server: LspServerDef,
	pub roots: Vec<String>,
}

/// Loads validated language-to-server mappings from the immutable registry catalog.
pub fn load_resolved_lsp_configs() -> Result<Vec<ResolvedLanguageLspConfig>> {
	let server_defs = load_lsp_configs()?;
	let servers_by_name: HashMap<_, _> = server_defs.into_iter().map(|server| (server.name.clone(), server)).collect();

	let mut resolved = Vec::new();
	for language in xeno_registry::LANGUAGES.snapshot_guard().iter_refs() {
		if language.lsp_servers.is_empty() {
			continue;
		}

		let roots = language.roots.iter().map(|&root| language.resolve(root).to_string()).collect::<Vec<_>>();

		let Some(server) = language
			.lsp_servers
			.iter()
			.find_map(|&server_sym| servers_by_name.get(language.resolve(server_sym)).cloned())
		else {
			continue;
		};

		resolved.push(ResolvedLanguageLspConfig {
			language: language.name_str().to_string(),
			server,
			roots,
		});
	}

	Ok(resolved)
}
