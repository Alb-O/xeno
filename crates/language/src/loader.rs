//! Language loader and registry.
//!
//! The [`LanguageLoader`] is the central registry for all language configurations.
//! It implements [`tree_house::LanguageLoader`] for injection handling.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

pub use tree_house::Language as LanguageId;
use tracing::error;
use tree_house::{InjectionLanguageMarker, Language, LanguageConfig as TreeHouseConfig};

use crate::config::load_language_configs;
use crate::language::LanguageData;

/// Simple glob pattern matching for file detection.
///
/// Supports `*` (any chars except `/`), `**` (any chars), `?` (single char).
fn glob_matches(pattern: &str, path: &str, filename: Option<&str>) -> bool {
	if !pattern.contains('/') {
		return filename.is_some_and(|f| glob_match_simple(pattern, f));
	}
	glob_match_simple(pattern, path)
}

fn glob_match_simple(pattern: &str, text: &str) -> bool {
	let mut p = pattern.chars().peekable();
	let mut t = text.chars().peekable();

	while let Some(pc) = p.next() {
		match pc {
			'*' => {
				if p.peek() == Some(&'*') {
					p.next();
					let remaining: String = p.collect();
					if remaining.is_empty() {
						return true;
					}
					let rest: String = t.collect();
					return (0..=rest.len()).any(|i| glob_match_simple(&remaining, &rest[i..]));
				}

				let remaining: String = p.collect();
				if remaining.is_empty() {
					return !t.any(|c| c == '/');
				}

				let rest: String = t.collect();
				for (i, c) in rest.char_indices() {
					if c == '/' {
						break;
					}
					if glob_match_simple(&remaining, &rest[i..]) {
						return true;
					}
				}
				return glob_match_simple(&remaining, "");
			}
			'?' if t.next().is_none() => return false,
			'?' => {}
			c if t.next() != Some(c) => return false,
			_ => {}
		}
	}

	t.next().is_none()
}

/// Central registry for all language configurations.
///
/// Handles language registration with file type associations, lazy loading of
/// grammars and queries, and lookup by filename, extension, shebang, or name.
#[derive(Debug, Default)]
pub struct LanguageLoader {
	languages: Vec<LanguageData>,
	by_extension: HashMap<String, usize>,
	by_filename: HashMap<String, usize>,
	globs: Vec<(String, usize)>,
	by_shebang: HashMap<String, usize>,
	by_name: HashMap<String, usize>,
}

impl LanguageLoader {
	/// Creates a new empty loader.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a loader populated from the embedded `languages.kdl`.
	pub fn from_embedded() -> Self {
		let mut loader = Self::new();
		match load_language_configs() {
			Ok(langs) => {
				for lang in langs {
					loader.register(lang);
				}
			}
			Err(e) => error!(error = %e, "Failed to load language configs"),
		}
		loader
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
		for glob in &data.globs {
			self.globs.push((glob.clone(), idx));
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
		let filename = path.file_name().and_then(|n| n.to_str());

		if let Some(name) = filename
			&& let Some(&idx) = self.by_filename.get(name)
		{
			return Some(Language::new(idx as u32));
		}

		if let Some(&idx) = path
			.extension()
			.and_then(|ext| ext.to_str())
			.and_then(|ext| self.by_extension.get(ext))
		{
			return Some(Language::new(idx as u32));
		}

		let path_str = path.to_string_lossy();
		for (pattern, idx) in &self.globs {
			if glob_matches(pattern, &path_str, filename) {
				return Some(Language::new(*idx as u32));
			}
		}

		None
	}

	/// Finds a language by shebang line.
	pub fn language_for_shebang(&self, first_line: &str) -> Option<Language> {
		if !first_line.starts_with("#!") {
			return None;
		}

		let line = first_line.trim_start_matches("#!");
		let parts: Vec<&str> = line.split_whitespace().collect();

		let interpreter = if parts.first() == Some(&"/usr/bin/env") || parts.first() == Some(&"env")
		{
			parts.get(1).copied()
		} else {
			parts.first().and_then(|p| p.rsplit('/').next())
		};

		interpreter.and_then(|interp| {
			let base = interp.trim_end_matches(|c: char| c.is_ascii_digit());
			self.by_shebang
				.get(base)
				.map(|&idx| Language::new(idx as u32))
		})
	}

	fn language_for_injection_match(&self, text: &str) -> Option<Language> {
		self.languages.iter().enumerate().find_map(|(idx, lang)| {
			lang.injection_regex
				.as_ref()
				.filter(|r| r.is_match(text))
				.map(|_| Language::new(idx as u32))
		})
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
				self.language_for_injection_match(&Cow::<str>::from(text))
			}
			InjectionLanguageMarker::Filename(text) => {
				self.language_for_path(Path::new(Cow::<str>::from(text).as_ref()))
			}
			InjectionLanguageMarker::Shebang(text) => {
				self.language_for_shebang(&Cow::<str>::from(text))
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
	fn loader_registration() {
		let mut loader = LanguageLoader::new();
		let data = LanguageData::new(
			"rust".to_string(),
			None,
			vec!["rs".to_string()],
			vec![],
			vec![],
			vec![],
			vec!["//".to_string()],
			Some(("/*".to_string(), "*/".to_string())),
			None,
		);

		let lang = loader.register(data);
		assert_eq!(lang.idx(), 0);
		assert_eq!(loader.language_for_path(Path::new("test.rs")), Some(lang));
		assert_eq!(loader.language_for_name("rust"), Some(lang));
	}

	#[test]
	fn shebang_detection() {
		let mut loader = LanguageLoader::new();
		let data = LanguageData::new(
			"python".to_string(),
			None,
			vec!["py".to_string()],
			vec![],
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
