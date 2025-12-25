//! Language configuration for syntax parsing.
//!
//! This module defines the configuration structures that connect file types
//! to their tree-sitter grammars and query files.

use std::collections::HashMap;
use std::path::Path;

/// Unique identifier for a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageId(pub u32);

impl LanguageId {
	pub const INVALID: LanguageId = LanguageId(u32::MAX);

	#[inline]
	pub fn is_valid(self) -> bool {
		self != Self::INVALID
	}

	#[inline]
	pub fn idx(self) -> usize {
		self.0 as usize
	}
}

/// Configuration for a single language.
#[derive(Debug, Clone)]
pub struct LanguageConfig {
	/// Language identifier (e.g., "rust", "python").
	pub language_id: String,

	/// Tree-sitter grammar name. Defaults to language_id if not specified.
	pub grammar: Option<String>,

	/// Regex for matching injection markers (e.g., in markdown code blocks).
	pub injection_regex: Option<regex::Regex>,

	/// File extensions associated with this language.
	pub extensions: Vec<String>,

	/// Exact filenames (e.g., "Makefile").
	pub filenames: Vec<String>,

	/// Shebang interpreters (e.g., "python", "bash").
	pub shebangs: Vec<String>,

	/// Comment token(s) for the language.
	pub comment_tokens: Vec<String>,

	/// Block comment tokens (start, end).
	pub block_comment: Option<(String, String)>,
}

impl LanguageConfig {
	/// Returns the grammar name to use for loading.
	pub fn grammar_name(&self) -> &str {
		self.grammar.as_deref().unwrap_or(&self.language_id)
	}
}

/// Manages language configurations and provides lookups.
#[derive(Debug, Default)]
pub struct LanguageLoader {
	languages: Vec<LanguageConfig>,
	by_extension: HashMap<String, LanguageId>,
	by_filename: HashMap<String, LanguageId>,
	by_shebang: HashMap<String, LanguageId>,
	scopes: Vec<String>,
}

impl LanguageLoader {
	pub fn new() -> Self {
		Self::default()
	}

	/// Registers a language configuration.
	pub fn register(&mut self, config: LanguageConfig) -> LanguageId {
		let id = LanguageId(self.languages.len() as u32);

		for ext in &config.extensions {
			self.by_extension.insert(ext.clone(), id);
		}

		for name in &config.filenames {
			self.by_filename.insert(name.clone(), id);
		}

		for shebang in &config.shebangs {
			self.by_shebang.insert(shebang.clone(), id);
		}

		self.languages.push(config);
		id
	}

	/// Gets a language configuration by ID.
	pub fn get(&self, id: LanguageId) -> Option<&LanguageConfig> {
		self.languages.get(id.idx())
	}

	/// Finds a language by name.
	pub fn language_for_name(&self, name: &str) -> Option<LanguageId> {
		self.languages
			.iter()
			.enumerate()
			.find(|(_, config)| config.language_id == name)
			.map(|(idx, _)| LanguageId(idx as u32))
	}

	/// Finds a language by file path (extension or exact filename).
	pub fn language_for_path(&self, path: &Path) -> Option<LanguageId> {
		// Check exact filename first
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			if let Some(id) = self.by_filename.get(name) {
				return Some(*id);
			}
		}

		path.extension()
			.and_then(|ext| ext.to_str())
			.and_then(|ext| self.by_extension.get(ext).copied())
	}

	/// Finds a language by shebang line.
	pub fn language_for_shebang(&self, first_line: &str) -> Option<LanguageId> {
		if !first_line.starts_with("#!") {
			return None;
		}

		let line = first_line.trim_start_matches("#!");
		let parts: Vec<&str> = line.split_whitespace().collect();

		// Handle /usr/bin/env python style
		let interpreter = if parts.first() == Some(&"/usr/bin/env") || parts.first() == Some(&"env")
		{
			parts.get(1).copied()
		} else {
			parts.first().and_then(|p| p.rsplit('/').next())
		};

		interpreter.and_then(|interp| {
			// Strip version numbers (python3 -> python)
			let base = interp.trim_end_matches(|c: char| c.is_ascii_digit());
			self.by_shebang.get(base).copied()
		})
	}

	/// Sets the highlight scopes (theme capture names).
	pub fn set_scopes(&mut self, scopes: Vec<String>) {
		self.scopes = scopes;
	}

	/// Returns the configured scopes.
	pub fn scopes(&self) -> &[String] {
		&self.scopes
	}

	/// Returns all registered languages.
	pub fn languages(&self) -> impl Iterator<Item = (LanguageId, &LanguageConfig)> {
		self.languages
			.iter()
			.enumerate()
			.map(|(idx, config)| (LanguageId(idx as u32), config))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_language_id() {
		let id = LanguageId(0);
		assert!(id.is_valid());
		assert_eq!(id.idx(), 0);
		assert!(!LanguageId::INVALID.is_valid());
	}

	#[test]
	fn test_loader_registration() {
		let mut loader = LanguageLoader::new();

		let config = LanguageConfig {
			language_id: "rust".to_string(),
			grammar: None,
			injection_regex: None,
			extensions: vec!["rs".to_string()],
			filenames: vec![],
			shebangs: vec![],
			comment_tokens: vec!["//".to_string()],
			block_comment: Some(("/*".to_string(), "*/".to_string())),
		};

		let id = loader.register(config);
		assert!(id.is_valid());
		assert_eq!(id.idx(), 0);

		let found = loader.language_for_path(Path::new("test.rs"));
		assert_eq!(found, Some(id));

		let found = loader.language_for_name("rust");
		assert_eq!(found, Some(id));
	}

	#[test]
	fn test_shebang_detection() {
		let mut loader = LanguageLoader::new();

		let config = LanguageConfig {
			language_id: "python".to_string(),
			grammar: None,
			injection_regex: None,
			extensions: vec!["py".to_string()],
			filenames: vec![],
			shebangs: vec!["python".to_string()],
			comment_tokens: vec!["#".to_string()],
			block_comment: None,
		};

		let id = loader.register(config);

		assert_eq!(loader.language_for_shebang("#!/usr/bin/python"), Some(id));
		assert_eq!(
			loader.language_for_shebang("#!/usr/bin/env python"),
			Some(id)
		);
		assert_eq!(loader.language_for_shebang("#!/usr/bin/python3"), Some(id));
		assert_eq!(loader.language_for_shebang("not a shebang"), None);
	}
}
