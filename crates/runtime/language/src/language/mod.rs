//! Language data definitions.
//!
//! This module contains the core `LanguageData` type that holds metadata
//! about a language, including file associations and lazily-loaded syntax config.

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::warn;
use tree_house::LanguageConfig as TreeHouseConfig;
use xeno_registry::themes::SyntaxStyles;

use crate::grammar::load_grammar_or_build;
use crate::query::read_query;

/// Serializable language data for build-time compilation.
///
/// Wire format for precompiled configs. Stores injection regex as string
/// (compiled to [`regex::Regex`] when converted to [`LanguageData`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDataRaw {
	pub name: String,
	pub grammar_name: String,
	pub extensions: Vec<String>,
	pub filenames: Vec<String>,
	pub globs: Vec<String>,
	pub shebangs: Vec<String>,
	pub comment_tokens: Vec<String>,
	pub block_comment: Option<(String, String)>,
	pub injection_regex: Option<String>,
	pub lsp_servers: Vec<String>,
	pub roots: Vec<String>,
}

impl From<LanguageDataRaw> for LanguageData {
	fn from(raw: LanguageDataRaw) -> Self {
		Self::new(
			raw.name,
			if raw.grammar_name.is_empty() {
				None
			} else {
				Some(raw.grammar_name)
			},
			raw.extensions,
			raw.filenames,
			raw.globs,
			raw.shebangs,
			raw.comment_tokens,
			raw.block_comment,
			raw.injection_regex.as_deref(),
			raw.lsp_servers,
			raw.roots,
		)
	}
}

impl From<&LanguageData> for LanguageDataRaw {
	fn from(data: &LanguageData) -> Self {
		Self {
			name: data.name.clone(),
			grammar_name: data.grammar_name.clone(),
			extensions: data.extensions.clone(),
			filenames: data.filenames.clone(),
			globs: data.globs.clone(),
			shebangs: data.shebangs.clone(),
			comment_tokens: data.comment_tokens.clone(),
			block_comment: data.block_comment.clone(),
			injection_regex: data
				.injection_regex
				.as_ref()
				.map(|r| r.as_str().to_string()),
			lsp_servers: data.lsp_servers.clone(),
			roots: data.roots.clone(),
		}
	}
}

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
	/// LSP server names (in priority order).
	pub lsp_servers: Vec<String>,
	/// Root markers for project detection (e.g., `Cargo.toml`).
	pub roots: Vec<String>,
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
		lsp_servers: Vec<String>,
		roots: Vec<String>,
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
			lsp_servers,
			roots,
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
mod tests;
