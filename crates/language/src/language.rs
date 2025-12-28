//! Language data definitions.
//!
//! This module contains the core `LanguageData` type that holds metadata
//! about a language, including file associations and lazily-loaded syntax config.

use once_cell::sync::OnceCell;
use evildoer_manifest::LanguageDef;
use tree_house::LanguageConfig as TreeHouseConfig;

use crate::grammar::load_grammar_or_build;
use crate::query::read_query;

/// Language data with lazily-loaded syntax configuration.
///
/// Each registered language has its grammar and queries loaded on first use.
#[derive(Debug)]
pub struct LanguageData {
	/// Language name (e.g., "rust", "python").
	pub name: String,
	/// Grammar name (may differ from language name).
	pub grammar_name: String,
	/// File extensions (without dot).
	pub extensions: Vec<String>,
	/// Exact filenames to match.
	pub filenames: Vec<String>,
	/// Shebang interpreters.
	pub shebangs: Vec<String>,
	/// Comment token(s) for the language.
	pub comment_tokens: Vec<String>,
	/// Block comment tokens (start, end).
	pub block_comment: Option<(String, String)>,
	/// Injection regex for matching in code blocks.
	pub injection_regex: Option<regex::Regex>,
	/// Lazily-loaded syntax configuration.
	config: OnceCell<Option<TreeHouseConfig>>,
}

impl LanguageData {
	/// Creates new language data.
	pub fn new(
		name: String,
		grammar_name: Option<String>,
		extensions: Vec<String>,
		filenames: Vec<String>,
		shebangs: Vec<String>,
		comment_tokens: Vec<String>,
		block_comment: Option<(String, String)>,
		injection_regex: Option<&str>,
	) -> Self {
		Self {
			grammar_name: grammar_name.unwrap_or_else(|| name.clone()),
			name,
			extensions,
			filenames,
			shebangs,
			comment_tokens,
			block_comment,
			injection_regex: injection_regex.and_then(|r| {
				regex::Regex::new(r)
					.map_err(|e| log::warn!("Invalid injection regex '{}': {}", r, e))
					.ok()
			}),
			config: OnceCell::new(),
		}
	}

	/// Returns the syntax configuration, loading it if necessary.
	///
	/// This loads the grammar and compiles the queries on first access.
	/// Returns `None` if loading fails, with errors logged.
	pub fn syntax_config(&self) -> Option<&TreeHouseConfig> {
		self.config
			.get_or_init(|| self.load_syntax_config())
			.as_ref()
	}

	/// Loads the complete language configuration (grammar + queries).
	///
	/// This will automatically attempt to fetch and build the grammar if it's
	/// not found in any of the search paths (including Helix runtime directories).
	fn load_syntax_config(&self) -> Option<TreeHouseConfig> {
		let grammar = match load_grammar_or_build(&self.grammar_name) {
			Ok(g) => g,
			Err(e) => {
				log::warn!("Failed to load grammar '{}': {}", self.grammar_name, e);
				return None;
			}
		};

		let highlights = read_query(&self.grammar_name, "highlights.scm");
		let injections = read_query(&self.grammar_name, "injections.scm");
		let locals = read_query(&self.grammar_name, "locals.scm");

		match TreeHouseConfig::new(grammar, &highlights, &injections, &locals) {
			Ok(config) => {
				// Configure the highlight query with scope names.
				// This maps capture names (e.g., "keyword") to Highlight indices.
				// We assign sequential indices to each unique scope name.
				let mut scope_idx = 0u32;
				config.configure(|_scope| {
					scope_idx += 1;
					Some(tree_house::highlighter::Highlight::new(scope_idx))
				});
				Some(config)
			}
			Err(e) => {
				log::warn!(
					"Failed to create language config for '{}': {}",
					self.grammar_name,
					e
				);
				None
			}
		}
	}
}

impl From<&LanguageDef> for LanguageData {
	fn from(def: &LanguageDef) -> Self {
		Self::new(
			def.name.to_string(),
			def.grammar.map(|s: &str| s.to_string()),
			def.extensions
				.iter()
				.map(|s: &&str| s.to_string())
				.collect(),
			def.filenames.iter().map(|s: &&str| s.to_string()).collect(),
			def.shebangs.iter().map(|s: &&str| s.to_string()).collect(),
			def.comment_tokens
				.iter()
				.map(|s: &&str| s.to_string())
				.collect(),
			def.block_comment
				.map(|(s, e): (&str, &str)| (s.to_string(), e.to_string())),
			def.injection_regex,
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_language_data_creation() {
		let data = LanguageData::new(
			"rust".to_string(),
			None,
			vec!["rs".to_string()],
			vec!["Cargo.toml".to_string()],
			vec![],
			vec!["//".to_string()],
			Some(("/*".to_string(), "*/".to_string())),
			Some(r"^rust$"),
		);

		assert_eq!(data.name, "rust");
		assert_eq!(data.grammar_name, "rust");
		assert!(data.injection_regex.is_some());
	}

	#[test]
	fn test_grammar_name_override() {
		let data = LanguageData::new(
			"typescript".to_string(),
			Some("tsx".to_string()),
			vec!["ts".to_string()],
			vec![],
			vec![],
			vec!["//".to_string()],
			None,
			None,
		);

		assert_eq!(data.name, "typescript");
		assert_eq!(data.grammar_name, "tsx");
	}
}
