//! Grammar configuration types and loading from KDL.

use std::path::PathBuf;

use kdl::KdlDocument;

use super::GrammarBuildError;
use crate::grammar::{cache_dir, runtime_dir};

/// Grammar configuration from grammars.kdl.
#[derive(Debug, Clone)]
pub struct GrammarConfig {
	/// The grammar name (used for the output library name).
	pub grammar_id: String,
	/// The source location for the grammar.
	pub source: GrammarSource,
}

/// Source location for a grammar.
#[derive(Debug, Clone)]
pub enum GrammarSource {
	/// A local path to the grammar source.
	Local {
		/// Filesystem path to the grammar sources.
		path: String,
	},
	/// A git repository containing the grammar.
	Git {
		/// Git remote URL.
		remote: String,
		/// Git revision (commit hash, tag, or branch).
		revision: String,
		/// Optional subdirectory within the repository.
		subpath: Option<String>,
	},
}

/// Loads grammar configurations from the embedded `grammars.kdl`.
pub fn load_grammar_configs() -> super::Result<Vec<GrammarConfig>> {
	parse_grammar_configs(evildoer_runtime::language::grammars_kdl())
}

/// Parse grammar configurations from a KDL string.
///
/// Each grammar is a node where the node name is the grammar identifier:
/// ```kdl
/// // Git source:
/// rust {
///     source "https://github.com/tree-sitter/tree-sitter-rust"
///     rev "abc123"
///     subpath "optional/path"  // optional
/// }
///
/// // Local source:
/// custom {
///     path "/path/to/grammar"
/// }
/// ```
fn parse_grammar_configs(input: &str) -> super::Result<Vec<GrammarConfig>> {
	let doc: KdlDocument = input.parse()?;

	let mut grammars = Vec::new();

	for node in doc.nodes() {
		let name = node.name().value();

		let children = match node.children() {
			Some(c) => c,
			None => continue,
		};

		let source = if let Some(path_node) = children.get("path") {
			let path = path_node
				.entry(0)
				.and_then(|e| e.value().as_string())
				.ok_or_else(|| {
					GrammarBuildError::ConfigParse(format!(
						"grammar '{}' path node missing value",
						name
					))
				})?;
			GrammarSource::Local {
				path: path.to_string(),
			}
		} else {
			// Git source - requires source and rev children
			let source_node = children.get("source").ok_or_else(|| {
				GrammarBuildError::ConfigParse(format!(
					"grammar '{}' missing 'source' or 'path' child",
					name
				))
			})?;

			let remote = source_node
				.entry(0)
				.and_then(|e| e.value().as_string())
				.ok_or_else(|| {
					GrammarBuildError::ConfigParse(format!(
						"grammar '{}' source node missing URL value",
						name
					))
				})?;

			let rev_node = children.get("rev").ok_or_else(|| {
				GrammarBuildError::ConfigParse(format!("grammar '{}' missing 'rev' child", name))
			})?;

			let revision = rev_node
				.entry(0)
				.and_then(|e| e.value().as_string())
				.ok_or_else(|| {
					GrammarBuildError::ConfigParse(format!(
						"grammar '{}' rev node missing value",
						name
					))
				})?;

			let subpath = children
				.get("subpath")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_string())
				.map(|s| s.to_string());

			GrammarSource::Git {
				remote: remote.to_string(),
				revision: revision.to_string(),
				subpath,
			}
		};

		grammars.push(GrammarConfig {
			grammar_id: name.to_string(),
			source,
		});
	}

	Ok(grammars)
}

/// Get the directory where grammar sources are stored.
///
/// Grammar sources are stored in the cache directory since they can be
/// re-fetched at any time.
pub fn grammar_sources_dir() -> PathBuf {
	cache_dir()
		.unwrap_or_else(runtime_dir)
		.join("grammars")
		.join("sources")
}

/// Returns the directory where compiled grammars are stored.
pub fn grammar_lib_dir() -> PathBuf {
	if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR")
		&& let Some(workspace) = std::path::Path::new(&manifest).ancestors().nth(2)
	{
		return workspace.join("target").join("grammars");
	}

	cache_dir()
		.map(|c| c.join("grammars"))
		.unwrap_or_else(|| runtime_dir().join("grammars"))
}

/// Get the source directory for a grammar (where parser.c lives).
pub fn get_grammar_src_dir(grammar: &GrammarConfig) -> PathBuf {
	match &grammar.source {
		GrammarSource::Local { path } => PathBuf::from(path).join("src"),
		GrammarSource::Git { subpath, .. } => {
			let base = grammar_sources_dir().join(&grammar.grammar_id);
			match subpath {
				Some(sub) => base.join(sub).join("src"),
				None => base.join("src"),
			}
		}
	}
}

/// Get the library file extension for the current platform.
#[cfg(target_os = "windows")]
pub fn library_extension() -> &'static str {
	"dll"
}

/// Get the library file extension for the current platform.
#[cfg(target_os = "macos")]
pub fn library_extension() -> &'static str {
	"dylib"
}

/// Get the library file extension for the current platform.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn library_extension() -> &'static str {
	"so"
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_load_grammar_configs() {
		// Test that embedded grammars.kdl parses correctly
		let result = load_grammar_configs();
		assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

		let configs = result.unwrap();
		assert!(!configs.is_empty(), "No grammar configs found");

		// Check that rust grammar exists
		let rust = configs.iter().find(|c| c.grammar_id == "rust");
		assert!(rust.is_some(), "Rust grammar not found");

		let rust = rust.unwrap();
		match &rust.source {
			GrammarSource::Git { remote, .. } => {
				assert!(remote.contains("tree-sitter-rust"));
			}
			GrammarSource::Local { .. } => panic!("Expected git source for rust"),
		}
	}

	#[test]
	fn test_parse_git_grammar() {
		let kdl = r#"
rust {
    source "https://github.com/tree-sitter/tree-sitter-rust"
    rev "abc123"
}
"#;
		let configs = parse_grammar_configs(kdl).unwrap();
		assert_eq!(configs.len(), 1);
		assert_eq!(configs[0].grammar_id, "rust");
		assert!(matches!(configs[0].source, GrammarSource::Git { .. }));
	}

	#[test]
	fn test_parse_git_grammar_with_subpath() {
		let kdl = r#"
typescript {
    source "https://github.com/tree-sitter/tree-sitter-typescript"
    rev "abc123"
    subpath "typescript"
}
"#;
		let configs = parse_grammar_configs(kdl).unwrap();
		assert_eq!(configs.len(), 1);
		match &configs[0].source {
			GrammarSource::Git { subpath, .. } => {
				assert_eq!(subpath.as_deref(), Some("typescript"));
			}
			_ => panic!("Expected git source"),
		}
	}

	#[test]
	fn test_parse_local_grammar() {
		let kdl = r#"
custom {
    path "/path/to/grammar"
}
"#;
		let configs = parse_grammar_configs(kdl).unwrap();
		assert_eq!(configs.len(), 1);
		assert_eq!(configs[0].grammar_id, "custom");
		match &configs[0].source {
			GrammarSource::Local { path } => {
				assert_eq!(path, "/path/to/grammar");
			}
			_ => panic!("Expected local source"),
		}
	}

	#[test]
	fn test_library_extension() {
		let ext = library_extension();
		#[cfg(target_os = "linux")]
		assert_eq!(ext, "so");
		#[cfg(target_os = "macos")]
		assert_eq!(ext, "dylib");
		#[cfg(target_os = "windows")]
		assert_eq!(ext, "dll");
	}
}
