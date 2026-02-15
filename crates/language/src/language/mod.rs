//! Language metadata wrapper and syntax-config derivation helpers.

use tracing::warn;
use tree_house::LanguageConfig as TreeHouseConfig;
use xeno_registry::languages::registry::LanguageRef;
use xeno_registry::themes::SyntaxStyles;

use crate::grammar::load_grammar_or_build;
use crate::query::read_query;
use crate::syntax::{ViewportRepair, ViewportRepairRule};

/// Language data wrapper over a registry entry.
#[derive(Debug, Clone)]
pub struct LanguageData {
	pub entry: LanguageRef,
}

impl LanguageData {
	pub fn name(&self) -> &str {
		self.entry.name_str()
	}

	pub fn grammar_name(&self) -> &str {
		self.entry.grammar_name.map(|s| self.entry.resolve(s)).unwrap_or_else(|| self.entry.name_str())
	}

	pub fn extensions(&self) -> impl Iterator<Item = &str> {
		self.entry.extensions.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn filenames(&self) -> impl Iterator<Item = &str> {
		self.entry.filenames.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn globs(&self) -> impl Iterator<Item = &str> {
		self.entry.globs.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn shebangs(&self) -> impl Iterator<Item = &str> {
		self.entry.shebangs.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn comment_tokens(&self) -> impl Iterator<Item = &str> {
		self.entry.comment_tokens.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn block_comment(&self) -> Option<(&str, &str)> {
		self.entry.block_comment.map(|(s1, s2)| (self.entry.resolve(s1), self.entry.resolve(s2)))
	}

	pub fn injection_regex(&self) -> Option<regex::Regex> {
		self.entry
			.injection_regex
			.map(|s| self.entry.resolve(s))
			.and_then(|r| regex::Regex::new(r).map_err(|e| warn!(regex = r, error = %e, "Invalid injection regex")).ok())
	}

	pub fn lsp_servers(&self) -> impl Iterator<Item = &str> {
		self.entry.lsp_servers.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn roots(&self) -> impl Iterator<Item = &str> {
		self.entry.roots.iter().map(|&s| self.entry.resolve(s))
	}

	pub fn viewport_repair(&self) -> ViewportRepair {
		if let Some(repair) = &self.entry.viewport_repair {
			return ViewportRepair {
				enabled: repair.enabled,
				max_scan_bytes: repair.max_scan_bytes,
				prefer_real_closer: repair.prefer_real_closer,
				max_forward_search_bytes: repair.max_forward_search_bytes,
				rules: repair
					.rules
					.iter()
					.map(|rule| match rule {
						xeno_registry::languages::types::ViewportRepairRuleEntry::BlockComment { open, close, nestable } => ViewportRepairRule::BlockComment {
							open: self.entry.resolve(*open).to_string(),
							close: self.entry.resolve(*close).to_string(),
							nestable: *nestable,
						},
						xeno_registry::languages::types::ViewportRepairRuleEntry::String { quote, escape } => ViewportRepairRule::String {
							quote: self.entry.resolve(*quote).to_string(),
							escape: escape.map(|s| self.entry.resolve(s).to_string()),
						},
						xeno_registry::languages::types::ViewportRepairRuleEntry::LineComment { start } => ViewportRepairRule::LineComment {
							start: self.entry.resolve(*start).to_string(),
						},
					})
					.collect(),
			};
		}

		// Default derivation
		let mut rules = Vec::new();

		// Block comment
		if let Some((open, close)) = self.block_comment() {
			rules.push(ViewportRepairRule::BlockComment {
				open: open.to_string(),
				close: close.to_string(),
				nestable: false, // conservative default
			});
		}

		// Line comments
		for token in self.comment_tokens() {
			rules.push(ViewportRepairRule::LineComment { start: token.to_string() });
		}

		// Strings (common defaults)
		rules.push(ViewportRepairRule::String {
			quote: "\"".to_string(),
			escape: Some("\\".to_string()),
		});
		rules.push(ViewportRepairRule::String {
			quote: "'".to_string(),
			escape: Some("\\".to_string()),
		});

		ViewportRepair {
			enabled: true,
			max_scan_bytes: 256 * 1024,
			prefer_real_closer: true,
			max_forward_search_bytes: 32 * 1024,
			rules,
		}
	}

	/// Returns the syntax configuration, loading it if necessary.
	pub fn syntax_config(&self) -> Option<TreeHouseConfig> {
		load_syntax_config(&self.entry)
	}
}

pub(crate) fn load_syntax_config(entry: &LanguageRef) -> Option<TreeHouseConfig> {
	let grammar_name = match entry.grammar_name {
		Some(sym) => entry.resolve(sym),
		None => entry.name_str(),
	};
	let grammar = match load_grammar_or_build(grammar_name) {
		Ok(g) => g,
		Err(e) => {
			warn!(grammar = grammar_name, error = %e, "Failed to load grammar");
			return None;
		}
	};

	let query_lang = entry.name_str();
	let highlights = read_query(query_lang, "highlights.scm");
	let injections = read_query(query_lang, "injections.scm");
	let locals = read_query(query_lang, "locals.scm");

	match TreeHouseConfig::new(grammar, &highlights, &injections, &locals) {
		Ok(config) => {
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
				grammar = grammar_name,
				error = %e,
				"Failed to create language config"
			);
			None
		}
	}
}
