//! Language data definitions.
//!
//! This module contains the core `LanguageData` type that holds metadata
//! about a language, including file associations and lazily-loaded syntax config.

use once_cell::sync::OnceCell;
use tracing::warn;
use tree_house::LanguageConfig as TreeHouseConfig;
use xeno_registry_themes::SyntaxStyles;

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
	/// Glob patterns for matching.
	pub globs: Vec<String>,
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
	#[allow(
		clippy::too_many_arguments,
		reason = "builder pattern not feasible for language config construction"
	)]
	pub fn new(
		name: String,
		grammar_name: Option<String>,
		extensions: Vec<String>,
		filenames: Vec<String>,
		globs: Vec<String>,
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
			globs,
			shebangs,
			comment_tokens,
			block_comment,
			injection_regex: injection_regex.and_then(|r| {
				regex::Regex::new(r)
					.map_err(|e| warn!(regex = r, error = %e, "Invalid injection regex"))
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
				warn!(grammar = self.grammar_name, error = %e, "Failed to load grammar");
				return None;
			}
		};

		let query_lang = &self.name;
		let highlights = read_query(query_lang, "highlights.scm");
		let injections = read_query(query_lang, "injections.scm");
		let locals = read_query(query_lang, "locals.scm");

		match TreeHouseConfig::new(grammar, &highlights, &injections, &locals) {
			Ok(config) => {
				// Configure the highlight query with scope names.
				// This maps capture names (e.g., "keyword.control.import") to Highlight indices.
				//
				// We use prefix matching: find the longest recognized scope that is a
				// prefix of the capture name. This allows captures like "keyword.control.flow"
				// to match "keyword.control" if the former isn't explicitly recognized.
				let scope_names = SyntaxStyles::scope_names();
				config.configure(|capture_name| {
					let capture_parts: Vec<_> = capture_name.split('.').collect();

					let mut best_index = None;
					let mut best_match_len = 0;

					for (i, recognized_name) in scope_names.iter().enumerate() {
						let mut len = 0;
						let mut matches = true;

						for (j, part) in recognized_name.split('.').enumerate() {
							match capture_parts.get(j) {
								Some(&capture_part) if capture_part == part => len += 1,
								_ => {
									matches = false;
									break;
								}
							}
						}

						if matches && len > best_match_len {
							best_index = Some(i);
							best_match_len = len;
						}
					}

					best_index.map(|idx| tree_house::highlighter::Highlight::new(idx as u32))
				});
				Some(config)
			}
			Err(e) => {
				warn!(
					grammar = self.grammar_name,
					error = %e,
					"Failed to create language config"
				);
				None
			}
		}
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
			vec![],
			vec!["//".to_string()],
			None,
			None,
		);

		assert_eq!(data.name, "typescript");
		assert_eq!(data.grammar_name, "tsx");
	}

	/// Helper to test scope prefix matching (same algorithm as configure callback).
	fn find_best_scope_match<'a>(capture_name: &str, scope_names: &[&'a str]) -> Option<&'a str> {
		let capture_parts: Vec<_> = capture_name.split('.').collect();

		let mut best_index = None;
		let mut best_match_len = 0;

		for (i, recognized_name) in scope_names.iter().enumerate() {
			let mut len = 0;
			let mut matches = true;

			for (j, part) in recognized_name.split('.').enumerate() {
				match capture_parts.get(j) {
					Some(&capture_part) if capture_part == part => len += 1,
					_ => {
						matches = false;
						break;
					}
				}
			}

			if matches && len > best_match_len {
				best_index = Some(i);
				best_match_len = len;
			}
		}

		best_index.map(|i| scope_names[i])
	}

	#[test]
	fn test_scope_prefix_matching() {
		let scopes = &[
			"keyword",
			"keyword.control",
			"keyword.control.import",
			"markup.heading",
			"markup.heading.1",
			"string",
			"string.special",
		];

		// Exact match
		assert_eq!(
			find_best_scope_match("keyword.control", scopes),
			Some("keyword.control")
		);

		// Prefix match - more specific capture falls back to less specific scope
		assert_eq!(
			find_best_scope_match("keyword.control.flow", scopes),
			Some("keyword.control")
		);

		// Longer prefix wins
		assert_eq!(
			find_best_scope_match("keyword.control.import.default", scopes),
			Some("keyword.control.import")
		);

		// Falls back to base scope
		assert_eq!(
			find_best_scope_match("keyword.operator", scopes),
			Some("keyword")
		);

		// No match at all
		assert_eq!(find_best_scope_match("comment", scopes), None);

		// Markup test
		assert_eq!(
			find_best_scope_match("markup.heading.marker", scopes),
			Some("markup.heading")
		);

		// String special paths
		assert_eq!(
			find_best_scope_match("string.special.path", scopes),
			Some("string.special")
		);
	}
}
