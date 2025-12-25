//! Language loader and registry.
//!
//! The `LanguageLoader` is the central registry for all language configurations.
//! It implements `tree_house::LanguageLoader` for injection handling.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use tree_house::{InjectionLanguageMarker, Language, LanguageConfig as TreeHouseConfig};

use crate::language::LanguageData;

// Re-export tree_house::Language for convenience.
pub use tree_house::Language as LanguageId;

/// The main language loader that implements tree-house's LanguageLoader trait.
///
/// This is the central registry for all language configurations. It handles:
/// - Language registration with file type associations
/// - Lazy loading of grammars and queries
/// - Lookup by filename, extension, shebang, or name
#[derive(Debug, Default)]
pub struct LanguageLoader {
	/// All registered languages.
	languages: Vec<LanguageData>,
	/// Lookup by extension.
	by_extension: HashMap<String, usize>,
	/// Lookup by filename.
	by_filename: HashMap<String, usize>,
	/// Lookup by shebang.
	by_shebang: HashMap<String, usize>,
	/// Lookup by name.
	by_name: HashMap<String, usize>,
}

impl LanguageLoader {
	/// Creates a new empty loader.
	pub fn new() -> Self {
		Self::default()
	}

	/// Registers a language and returns its ID.
	pub fn register(&mut self, data: LanguageData) -> Language {
		let idx = self.languages.len();

		for ext in &data.extensions {
			self.by_extension.insert(ext.clone(), idx);
		}

		for fname in &data.filenames {
			self.by_filename.insert(fname.clone(), idx);
		}

		for shebang in &data.shebangs {
			self.by_shebang.insert(shebang.clone(), idx);
		}

		self.by_name.insert(data.name.clone(), idx);

		self.languages.push(data);
		Language::new(idx as u32)
	}

	/// Gets language data by ID.
	pub fn get(&self, lang: Language) -> Option<&LanguageData> {
		self.languages.get(lang.idx())
	}

	/// Finds a language by name.
	pub fn language_for_name(&self, name: &str) -> Option<Language> {
		self.by_name.get(name).map(|&idx| Language::new(idx as u32))
	}

	/// Finds a language by file path.
	pub fn language_for_path(&self, path: &Path) -> Option<Language> {
		// Check exact filename first
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			if let Some(&idx) = self.by_filename.get(name) {
				return Some(Language::new(idx as u32));
			}
		}

		// Check extension
		path.extension()
			.and_then(|ext| ext.to_str())
			.and_then(|ext| self.by_extension.get(ext))
			.map(|&idx| Language::new(idx as u32))
	}

	/// Finds a language by shebang line.
	pub fn language_for_shebang(&self, first_line: &str) -> Option<Language> {
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
			self.by_shebang
				.get(base)
				.map(|&idx| Language::new(idx as u32))
		})
	}

	/// Finds a language by injection regex match.
	fn language_for_injection_match(&self, text: &str) -> Option<Language> {
		for (idx, lang) in self.languages.iter().enumerate() {
			if let Some(ref regex) = lang.injection_regex {
				if regex.is_match(text) {
					return Some(Language::new(idx as u32));
				}
			}
		}
		None
	}

	/// Returns all registered languages.
	pub fn languages(&self) -> impl Iterator<Item = (Language, &LanguageData)> {
		self.languages
			.iter()
			.enumerate()
			.map(|(idx, data)| (Language::new(idx as u32), data))
	}

	/// Returns the number of registered languages.
	pub fn len(&self) -> usize {
		self.languages.len()
	}

	/// Returns true if no languages are registered.
	pub fn is_empty(&self) -> bool {
		self.languages.is_empty()
	}
}

impl tree_house::LanguageLoader for LanguageLoader {
	fn language_for_marker(&self, marker: InjectionLanguageMarker) -> Option<Language> {
		match marker {
			InjectionLanguageMarker::Name(name) => self.language_for_name(name),
			InjectionLanguageMarker::Match(text) => {
				let cow: Cow<str> = text.into();
				self.language_for_injection_match(&cow)
			}
			InjectionLanguageMarker::Filename(text) => {
				let path: Cow<str> = text.into();
				self.language_for_path(Path::new(path.as_ref()))
			}
			InjectionLanguageMarker::Shebang(text) => {
				let line: Cow<str> = text.into();
				self.language_for_shebang(&line)
			}
		}
	}

	fn get_config(&self, lang: Language) -> Option<&TreeHouseConfig> {
		self.languages.get(lang.idx())?.syntax_config()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_loader_registration() {
		let mut loader = LanguageLoader::new();

		let data = LanguageData::new(
			"rust".to_string(),
			None,
			vec!["rs".to_string()],
			vec![],
			vec![],
			vec!["//".to_string()],
			Some(("/*".to_string(), "*/".to_string())),
			None,
		);

		let lang = loader.register(data);
		assert_eq!(lang.idx(), 0);

		let found = loader.language_for_path(Path::new("test.rs"));
		assert_eq!(found, Some(lang));

		let found = loader.language_for_name("rust");
		assert_eq!(found, Some(lang));
	}

	#[test]
	fn test_shebang_detection() {
		let mut loader = LanguageLoader::new();

		let data = LanguageData::new(
			"python".to_string(),
			None,
			vec!["py".to_string()],
			vec![],
			vec!["python".to_string()],
			vec!["#".to_string()],
			None,
			None,
		);

		let lang = loader.register(data);

		assert_eq!(loader.language_for_shebang("#!/usr/bin/python"), Some(lang));
		assert_eq!(
			loader.language_for_shebang("#!/usr/bin/env python"),
			Some(lang)
		);
		assert_eq!(
			loader.language_for_shebang("#!/usr/bin/python3"),
			Some(lang)
		);
		assert_eq!(loader.language_for_shebang("not a shebang"), None);
	}
}
