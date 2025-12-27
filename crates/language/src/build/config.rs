//! Grammar configuration types and loading.

use std::path::PathBuf;

use serde::Deserialize;

use crate::grammar::{cache_dir, grammar_search_paths, runtime_dir};

/// Grammar configuration from languages.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct GrammarConfig {
	/// The grammar name (used for the output library name).
	#[serde(rename = "name")]
	pub grammar_id: String,
	/// The source location for the grammar.
	pub source: GrammarSource,
}

/// Source location for a grammar.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum GrammarSource {
	/// A local path to the grammar source.
	Local { path: String },
	/// A git repository containing the grammar.
	Git {
		#[serde(rename = "git")]
		remote: String,
		#[serde(rename = "rev")]
		revision: String,
		/// Optional subdirectory within the repository.
		subpath: Option<String>,
	},
}

/// Languages configuration file structure.
#[derive(Debug, Deserialize)]
pub(super) struct LanguagesConfig {
	#[serde(default)]
	pub grammar: Vec<GrammarConfig>,
}

/// Embedded languages.toml from the runtime directory.
const LANGUAGES_TOML: &str = include_str!("../../../../runtime/languages.toml");

/// Loads grammar configurations from the embedded `languages.toml`.
pub fn load_grammar_configs() -> super::Result<Vec<GrammarConfig>> {
	let config: LanguagesConfig = toml::from_str(LANGUAGES_TOML)?;
	Ok(config.grammar)
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

/// Get the directory where compiled grammars are stored.
pub fn grammar_lib_dir() -> PathBuf {
	// Use the first grammar search path, or fall back to runtime/grammars
	grammar_search_paths()
		.first()
		.cloned()
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

#[cfg(target_os = "macos")]
pub fn library_extension() -> &'static str {
	"dylib"
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn library_extension() -> &'static str {
	"so"
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_load_grammar_configs() {
		// This test will pass if languages.toml doesn't exist or is valid
		let result = load_grammar_configs();
		assert!(result.is_ok());
	}

	#[test]
	fn test_grammar_source_deserialization() {
		let toml_git = r#"
            [[grammar]]
            name = "rust"
            source = { git = "https://github.com/tree-sitter/tree-sitter-rust", rev = "abc123" }
        "#;

		let config: LanguagesConfig = toml::from_str(toml_git).unwrap();
		assert_eq!(config.grammar.len(), 1);
		assert_eq!(config.grammar[0].grammar_id, "rust");
		assert!(matches!(
			config.grammar[0].source,
			GrammarSource::Git { .. }
		));

		let toml_local = r#"
            [[grammar]]
            name = "custom"
            source = { path = "/path/to/grammar" }
        "#;

		let config: LanguagesConfig = toml::from_str(toml_local).unwrap();
		assert_eq!(config.grammar.len(), 1);
		assert!(matches!(
			config.grammar[0].source,
			GrammarSource::Local { .. }
		));
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
