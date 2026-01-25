//! LSP server configuration loading.
//!
//! Server definitions are loaded from bincode blobs compiled at build time.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

#[cfg(test)]
mod tests;

/// Errors that can occur when loading LSP configurations.
#[derive(Debug, Error)]
pub enum LspConfigError {
	#[error("failed to deserialize precompiled data: {0}")]
	Bincode(#[from] bincode::Error),
	#[error("invalid precompiled blob (magic/version mismatch)")]
	InvalidBlob,
}

/// Result type for LSP configuration operations.
pub type Result<T> = std::result::Result<T, LspConfigError>;

static LSP_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lsp.bin"));

/// LSP server definition parsed from `lsp.kdl`.
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

/// Serializable LSP server configuration for build-time compilation.
///
/// Stores config as JSON string (bincode doesn't support `serde_json::Value`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerDefRaw {
	pub name: String,
	pub command: String,
	pub args: Vec<String>,
	pub environment: HashMap<String, String>,
	pub config_json: Option<String>,
	pub source: Option<String>,
	pub nix: Option<String>,
}

impl From<LspServerDefRaw> for LspServerDef {
	fn from(raw: LspServerDefRaw) -> Self {
		Self {
			name: raw.name,
			command: raw.command,
			args: raw.args,
			environment: raw.environment,
			config: raw.config_json.and_then(|s| serde_json::from_str(&s).ok()),
			source: raw.source,
			nix: raw.nix,
		}
	}
}

impl From<&LspServerDef> for LspServerDefRaw {
	fn from(def: &LspServerDef) -> Self {
		Self {
			name: def.name.clone(),
			command: def.command.clone(),
			args: def.args.clone(),
			environment: def.environment.clone(),
			config_json: def.config.as_ref().map(|v| v.to_string()),
			source: def.source.clone(),
			nix: def.nix.clone(),
		}
	}
}

/// Loads LSP server configurations from precompiled bincode.
pub fn load_lsp_configs() -> Result<Vec<LspServerDef>> {
	let payload = crate::precompiled::validate_blob(LSP_BIN).ok_or(LspConfigError::InvalidBlob)?;
	let raw: Vec<LspServerDefRaw> = bincode::deserialize(payload)?;
	Ok(raw.into_iter().map(LspServerDef::from).collect())
}

/// Language LSP configuration extracted from languages.kdl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageLspInfo {
	/// LSP server names for this language.
	pub servers: Vec<String>,
	/// Root markers for project detection.
	pub roots: Vec<String>,
}

/// Mapping of language name to its LSP configuration.
pub type LanguageLspMapping = HashMap<String, LanguageLspInfo>;

// Test-only KDL parsing functions
#[cfg(test)]
mod parsing {
	use kdl::{KdlDocument, KdlNode};

	use super::*;
	use crate::utils::parse_string_args;

	#[derive(Debug, thiserror::Error)]
	pub enum ParseError {
		#[error("failed to parse KDL: {0}")]
		Kdl(#[from] kdl::KdlError),
	}

	pub fn parse_lsp_configs(input: &str) -> std::result::Result<Vec<LspServerDef>, ParseError> {
		let doc: KdlDocument = input.parse()?;
		Ok(doc.nodes().iter().map(parse_server_node).collect())
	}

	fn parse_server_node(node: &KdlNode) -> LspServerDef {
		let name = node.name().value().to_string();
		let command = node
			.entry(0)
			.and_then(|e| {
				if e.name().is_none() {
					e.value().as_string().map(String::from)
				} else {
					None
				}
			})
			.unwrap_or_else(|| name.clone());

		let children = node.children();
		let args = parse_string_args(children, "args");
		let environment = parse_environment(children);
		let config = children.and_then(|c| c.get("config")).map(kdl_node_to_json);
		let source = children
			.and_then(|c| c.get("source"))
			.and_then(|n| n.entry(0))
			.and_then(|e| e.value().as_string())
			.map(String::from);
		let nix = children.and_then(|c| c.get("nix")).and_then(|n| {
			let entry = n.entry(0)?;
			if entry.value().as_bool() == Some(false) {
				None
			} else {
				entry.value().as_string().map(String::from)
			}
		});

		LspServerDef {
			name,
			command,
			args,
			environment,
			config,
			source,
			nix,
		}
	}

	fn parse_environment(children: Option<&KdlDocument>) -> HashMap<String, String> {
		let mut env = HashMap::new();
		let Some(env_node) = children.and_then(|c| c.get("environment")) else {
			return env;
		};

		for entry in env_node.entries() {
			if let Some(name) = entry.name()
				&& let Some(value) = entry.value().as_string()
			{
				env.insert(name.value().to_string(), value.to_string());
			}
		}

		if let Some(env_children) = env_node.children() {
			for child in env_children.nodes() {
				if let Some(value) = child.entry(0).and_then(|e| e.value().as_string()) {
					env.insert(child.name().value().to_string(), value.to_string());
				}
			}
		}

		env
	}

	fn kdl_node_to_json(node: &KdlNode) -> JsonValue {
		let Some(children) = node.children() else {
			if let Some(entry) = node.entry(0) {
				return kdl_value_to_json(entry.value());
			}
			return JsonValue::Object(serde_json::Map::new());
		};
		kdl_doc_to_json(children)
	}

	fn kdl_doc_to_json(doc: &KdlDocument) -> JsonValue {
		let mut map = serde_json::Map::new();
		for node in doc.nodes() {
			let key = node.name().value().to_string();
			if node.children().is_some() {
				map.insert(key, kdl_node_to_json(node));
			} else if let Some(entry) = node.entry(0) {
				map.insert(key, kdl_value_to_json(entry.value()));
			} else {
				map.insert(key, JsonValue::Bool(true));
			}
		}
		JsonValue::Object(map)
	}

	fn kdl_value_to_json(value: &kdl::KdlValue) -> JsonValue {
		if let Some(s) = value.as_string() {
			JsonValue::String(s.to_string())
		} else if let Some(i) = value.as_integer() {
			JsonValue::Number((i as i64).into())
		} else if let Some(b) = value.as_bool() {
			JsonValue::Bool(b)
		} else {
			JsonValue::Null
		}
	}

	pub fn parse_language_lsp_mapping(
		input: &str,
	) -> std::result::Result<LanguageLspMapping, ParseError> {
		let doc: KdlDocument = input.parse()?;
		let mut mapping = HashMap::new();

		for node in doc.nodes() {
			if node.name().value() != "language" {
				continue;
			}

			let Some(name) = node.get("name").and_then(|v| v.as_string()) else {
				continue;
			};

			let servers = parse_language_servers(node);
			let roots = parse_string_args(node.children(), "roots");

			if !servers.is_empty() {
				mapping.insert(name.to_string(), LanguageLspInfo { servers, roots });
			}
		}

		Ok(mapping)
	}

	fn parse_language_servers(node: &KdlNode) -> Vec<String> {
		let Some(children) = node.children() else {
			return Vec::new();
		};

		let Some(ls_node) = children.get("language-servers") else {
			return Vec::new();
		};

		let inline: Vec<String> = ls_node
			.entries()
			.iter()
			.filter(|e| e.name().is_none())
			.filter_map(|e| e.value().as_string())
			.map(String::from)
			.collect();

		if !inline.is_empty() {
			return inline;
		}

		let Some(ls_children) = ls_node.children() else {
			return Vec::new();
		};

		ls_children
			.nodes()
			.iter()
			.filter(|n| n.name().value() == "-")
			.filter_map(|n| n.get("name").and_then(|v| v.as_string()).map(String::from))
			.collect()
	}
}

#[cfg(test)]
pub use parsing::{parse_language_lsp_mapping, parse_lsp_configs};
