//! LSP server configuration loading.
//!
//! Server definitions are loaded from registry specs.

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

/// LSP server definition loaded from registry specs.
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

/// Loads LSP server configurations from registry specs.
pub fn load_lsp_configs() -> Result<Vec<LspServerDef>> {
	let spec = xeno_registry::domains::lsp_servers::loader::load_lsp_servers_spec();
	Ok(spec
		.servers
		.into_iter()
		.map(|raw| LspServerDef {
			name: raw.common.name,
			command: raw.command,
			args: raw.args,
			environment: raw.environment.into_iter().collect(),
			config: raw.config_json.and_then(|s| serde_json::from_str(&s).ok()),
			source: raw.source,
			nix: raw.nix,
		})
		.collect())
}

/// Language LSP configuration extracted from registry specs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LanguageLspInfo {
	/// LSP server names for this language.
	pub servers: Vec<String>,
	/// Root markers for project detection.
	pub roots: Vec<String>,
}

/// Mapping of language name to its LSP configuration.
pub type LanguageLspMapping = HashMap<String, LanguageLspInfo>;
