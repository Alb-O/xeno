//! LSP server configuration parsing from KDL.
//!
//! Parses `lsp.kdl` to extract language server definitions including commands,
//! arguments, environment variables, and initialization options.
//!
//! Also parses `languages.kdl` to extract language-to-server mappings.
//!
//! # KDL Format
//!
//! ```kdl
//! // Simple server (command = server name)
//! rust-analyzer {
//!     source "https://github.com/rust-lang/rust-analyzer"
//!     nix rust-analyzer
//!     config {
//!         inlayHints {
//!             enable #true
//!         }
//!     }
//! }
//!
//! // Server with different command
//! angular ngserver {
//!     args --stdio --tsProbeLocations . --ngProbeLocations .
//!     source "https://github.com/angular/vscode-ng-language-service"
//!     nix angular-language-server
//! }
//! ```

use std::collections::HashMap;

use kdl::{KdlDocument, KdlNode};
use serde_json::Value as JsonValue;
use thiserror::Error;

/// Errors from LSP configuration parsing.
#[derive(Debug, Error)]
pub enum LspConfigError {
	/// KDL syntax error.
	#[error("failed to parse KDL: {0}")]
	KdlParse(#[from] kdl::KdlError),
}

/// Result type for LSP configuration operations.
pub type Result<T> = std::result::Result<T, LspConfigError>;

/// A parsed LSP server configuration.
#[derive(Debug, Clone)]
pub struct LspServerDef {
	/// Server identifier (node name in KDL).
	pub name: String,
	/// Command to execute (may differ from name).
	pub command: String,
	/// Command-line arguments.
	pub args: Vec<String>,
	/// Environment variables.
	pub environment: HashMap<String, String>,
	/// Initialization options (config block).
	pub config: Option<JsonValue>,
	/// Source URL (for documentation).
	pub source: Option<String>,
	/// Nix package attribute (for installation hints).
	pub nix: Option<String>,
}

/// Loads LSP server configurations from the embedded `lsp.kdl`.
pub fn load_lsp_configs() -> Result<Vec<LspServerDef>> {
	parse_lsp_configs(xeno_runtime::language::lsp_kdl())
}

/// Parses LSP server configurations from a KDL string.
pub fn parse_lsp_configs(input: &str) -> Result<Vec<LspServerDef>> {
	let doc: KdlDocument = input.parse()?;
	let mut servers = Vec::new();

	for node in doc.nodes() {
		servers.push(parse_server_node(node));
	}

	Ok(servers)
}

/// Parses a single server node into LspServerDef.
fn parse_server_node(node: &KdlNode) -> LspServerDef {
	let name = node.name().value().to_string();

	// Command is either the first positional arg or the server name
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

/// Extracts string arguments from a named child node.
fn parse_string_args(children: Option<&KdlDocument>, name: &str) -> Vec<String> {
	children
		.and_then(|c| c.get(name))
		.map(|node| {
			node.entries()
				.iter()
				.filter(|e| e.name().is_none())
				.filter_map(|e| e.value().as_string())
				.map(String::from)
				.collect()
		})
		.unwrap_or_default()
}

/// Parses environment variables from the environment block.
fn parse_environment(children: Option<&KdlDocument>) -> HashMap<String, String> {
	let mut env = HashMap::new();

	let Some(env_node) = children.and_then(|c| c.get("environment")) else {
		return env;
	};

	// Environment can be key=value entries on the node itself
	for entry in env_node.entries() {
		if let Some(name) = entry.name()
			&& let Some(value) = entry.value().as_string()
		{
			env.insert(name.value().to_string(), value.to_string());
		}
	}

	// Or children nodes with values
	if let Some(env_children) = env_node.children() {
		for child in env_children.nodes() {
			if let Some(value) = child.entry(0).and_then(|e| e.value().as_string()) {
				env.insert(child.name().value().to_string(), value.to_string());
			}
		}
	}

	env
}

/// Converts a KDL node's children into a JSON value.
fn kdl_node_to_json(node: &KdlNode) -> JsonValue {
	let Some(children) = node.children() else {
		// No children - check for a direct value
		if let Some(entry) = node.entry(0) {
			return kdl_value_to_json(entry.value());
		}
		return JsonValue::Object(serde_json::Map::new());
	};

	kdl_doc_to_json(children)
}

/// Converts a KDL document into a JSON object.
fn kdl_doc_to_json(doc: &KdlDocument) -> JsonValue {
	let mut map = serde_json::Map::new();

	for node in doc.nodes() {
		let key = node.name().value().to_string();

		// Check if it has children (nested object)
		if node.children().is_some() {
			map.insert(key, kdl_node_to_json(node));
		} else if let Some(entry) = node.entry(0) {
			// Single value
			map.insert(key, kdl_value_to_json(entry.value()));
		} else {
			// Empty node - treat as true (presence = enabled)
			map.insert(key, JsonValue::Bool(true));
		}
	}

	JsonValue::Object(map)
}

/// Converts a KDL value to a JSON value.
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

/// Language LSP configuration extracted from languages.kdl.
#[derive(Debug, Clone)]
pub struct LanguageLspInfo {
	/// LSP server names for this language.
	pub servers: Vec<String>,
	/// Root markers for project detection.
	pub roots: Vec<String>,
}

/// Mapping of language name to its LSP configuration.
pub type LanguageLspMapping = HashMap<String, LanguageLspInfo>;

/// Loads language-to-LSP server mappings from the embedded `languages.kdl`.
pub fn load_language_lsp_mapping() -> Result<LanguageLspMapping> {
	parse_language_lsp_mapping(xeno_runtime::language::languages_kdl())
}

/// Parses language-to-LSP server mappings from a KDL string.
pub fn parse_language_lsp_mapping(input: &str) -> Result<LanguageLspMapping> {
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

		// Only include languages that have LSP servers configured
		if !servers.is_empty() {
			mapping.insert(name.to_string(), LanguageLspInfo { servers, roots });
		}
	}

	Ok(mapping)
}

/// Parses the `language-servers` field from a language node.
fn parse_language_servers(node: &KdlNode) -> Vec<String> {
	let Some(children) = node.children() else {
		return Vec::new();
	};

	let Some(ls_node) = children.get("language-servers") else {
		return Vec::new();
	};

	// Try inline args first: `language-servers rust-analyzer`
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

	// Try block format: `language-servers { - name="..." }`
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_simple_server() {
		let kdl = r#"
rust-analyzer {
    source "https://github.com/rust-lang/rust-analyzer"
    nix rust-analyzer
}
"#;
		let servers = parse_lsp_configs(kdl).unwrap();
		assert_eq!(servers.len(), 1);

		let ra = &servers[0];
		assert_eq!(ra.name, "rust-analyzer");
		assert_eq!(ra.command, "rust-analyzer");
		assert!(ra.args.is_empty());
		assert_eq!(
			ra.source,
			Some("https://github.com/rust-lang/rust-analyzer".to_string())
		);
		assert_eq!(ra.nix, Some("rust-analyzer".to_string()));
	}

	#[test]
	fn parse_server_with_different_command() {
		let kdl = r#"
angular ngserver {
    args --stdio --tsProbeLocations .
    source "https://github.com/angular/vscode-ng-language-service"
    nix angular-language-server
}
"#;
		let servers = parse_lsp_configs(kdl).unwrap();
		let angular = &servers[0];

		assert_eq!(angular.name, "angular");
		assert_eq!(angular.command, "ngserver");
		assert_eq!(angular.args, vec!["--stdio", "--tsProbeLocations", "."]);
		assert_eq!(angular.nix, Some("angular-language-server".to_string()));
	}

	#[test]
	fn parse_server_with_config() {
		let kdl = r#"
typescript-language-server {
    args --stdio
    config {
        hostInfo helix
        typescript {
            inlayHints {
                enable #true
            }
        }
    }
}
"#;
		let servers = parse_lsp_configs(kdl).unwrap();
		let ts = &servers[0];

		assert!(ts.config.is_some());
		let config = ts.config.as_ref().unwrap();
		assert_eq!(config["hostInfo"], "helix");
		assert_eq!(config["typescript"]["inlayHints"]["enable"], true);
	}

	#[test]
	fn parse_server_with_nix_false() {
		let kdl = r#"
my-server {
    nix #false
}
"#;
		let servers = parse_lsp_configs(kdl).unwrap();
		assert!(servers[0].nix.is_none());
	}

	#[test]
	fn load_embedded_lsp_configs() {
		let servers = load_lsp_configs().expect("embedded lsp.kdl should parse");
		assert!(!servers.is_empty());

		// Check rust-analyzer exists
		let ra = servers
			.iter()
			.find(|s| s.name == "rust-analyzer")
			.expect("rust-analyzer should exist");
		assert_eq!(ra.command, "rust-analyzer");

		// Check typescript-language-server exists
		let ts = servers
			.iter()
			.find(|s| s.name == "typescript-language-server")
			.expect("typescript-language-server should exist");
		assert_eq!(ts.args, vec!["--stdio"]);
	}

	#[test]
	fn parse_language_lsp_mapping_inline() {
		let kdl = r#"
language name=rust scope=source.rust {
    file-types rs
    roots Cargo.toml Cargo.lock
    language-servers rust-analyzer
}
language name=toml scope=source.toml {
    file-types toml
    language-servers taplo tombi
}
"#;
		let mapping = parse_language_lsp_mapping(kdl).unwrap();

		let rust = mapping.get("rust").unwrap();
		assert_eq!(rust.servers, vec!["rust-analyzer"]);
		assert_eq!(rust.roots, vec!["Cargo.toml", "Cargo.lock"]);

		let toml = mapping.get("toml").unwrap();
		assert_eq!(toml.servers, vec!["taplo", "tombi"]);
		assert!(toml.roots.is_empty());
	}

	#[test]
	fn load_embedded_language_lsp_mapping() {
		let mapping = load_language_lsp_mapping().expect("embedded languages.kdl should parse");
		assert!(!mapping.is_empty());

		// Check rust has rust-analyzer
		let rust = mapping.get("rust").expect("rust should have servers");
		assert!(rust.servers.contains(&"rust-analyzer".to_string()));
		assert!(rust.roots.contains(&"Cargo.toml".to_string()));

		// Check python has servers
		let python = mapping.get("python").expect("python should have servers");
		assert!(!python.servers.is_empty());
	}
}
