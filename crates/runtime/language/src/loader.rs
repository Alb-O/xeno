//! Language loader and registry.
//!
//! The [`LanguageLoader`] is a thin wrapper over [`LanguageDb`] that implements
//! [`tree_house::LanguageLoader`] for injection handling.
//!
//! [`LanguageDb`]: crate::db::LanguageDb

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

pub use tree_house::Language as LanguageId;
use tree_house::{InjectionLanguageMarker, Language, LanguageConfig as TreeHouseConfig};

use crate::db::{LanguageDb, language_db};
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

/// Matches a simple glob pattern against text without path separators.
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

/// Wrapper over [`LanguageDb`] implementing `tree_house::LanguageLoader`.
///
/// Use [`from_embedded()`](Self::from_embedded) to create a loader backed by
/// the global language database (parsed once from `languages.kdl`).
#[derive(Debug, Clone)]
pub struct LanguageLoader {
	db: Arc<LanguageDb>,
}

impl Default for LanguageLoader {
	fn default() -> Self {
		Self::from_embedded()
	}
}

impl LanguageLoader {
	/// Creates an empty loader.
	///
	/// Use [`from_embedded()`](Self::from_embedded) for a loader backed by the
	/// global language database.
	pub fn new() -> Self {
		Self {
			db: Arc::new(LanguageDb::new()),
		}
	}

	/// Creates a loader backed by the global language database.
	pub fn from_embedded() -> Self {
		Self {
			db: Arc::clone(language_db()),
		}
	}

	/// Creates a loader from a custom database.
	pub fn from_db(db: Arc<LanguageDb>) -> Self {
		Self { db }
	}

	/// Gets language data by ID.
	pub fn get(&self, lang: Language) -> Option<&LanguageData> {
		self.db.get(lang.idx())
	}

	/// Finds a language by name.
	pub fn language_for_name(&self, name: &str) -> Option<Language> {
		self.db
			.index_for_name(name)
			.map(|idx| Language::new(idx as u32))
	}

	/// Finds a language by file path.
	pub fn language_for_path(&self, path: &Path) -> Option<Language> {
		let filename = path.file_name().and_then(|n| n.to_str());

		if let Some(name) = filename
			&& let Some(idx) = self.db.index_for_filename(name)
		{
			return Some(Language::new(idx as u32));
		}

		if let Some(idx) = path
			.extension()
			.and_then(|ext| ext.to_str())
			.and_then(|ext| self.db.index_for_extension(ext))
		{
			return Some(Language::new(idx as u32));
		}

		let path_str = path.to_string_lossy();
		for (pattern, idx) in self.db.globs() {
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
			self.db
				.index_for_shebang(base)
				.map(|idx| Language::new(idx as u32))
		})
	}

	/// Finds a language by matching text against injection regexes.
	fn language_for_injection_match(&self, text: &str) -> Option<Language> {
		self.db.languages().find_map(|(idx, lang)| {
			lang.injection_regex
				.as_ref()
				.filter(|r| r.is_match(text))
				.map(|_| Language::new(idx as u32))
		})
	}

	/// Returns all registered languages.
	pub fn languages(&self) -> impl Iterator<Item = (Language, &LanguageData)> {
		self.db
			.languages()
			.map(|(idx, data)| (Language::new(idx as u32), data))
	}

	/// Returns the number of registered languages.
	pub fn len(&self) -> usize {
		self.db.len()
	}

	/// Returns true if no languages are registered.
	pub fn is_empty(&self) -> bool {
		self.db.is_empty()
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
		self.db.get(lang.idx())?.syntax_config()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn test_db() -> Arc<LanguageDb> {
		let mut db = LanguageDb::new();
		db.register(LanguageData::new(
			"rust".to_string(),
			None,
			vec!["rs".to_string()],
			vec![],
			vec![],
			vec![],
			vec!["//".to_string()],
			Some(("/*".to_string(), "*/".to_string())),
			None,
			vec![],
			vec![],
		));
		db.register(LanguageData::new(
			"python".to_string(),
			None,
			vec!["py".to_string()],
			vec![],
			vec![],
			vec!["python".to_string()],
			vec!["#".to_string()],
			None,
			None,
			vec![],
			vec![],
		));
		Arc::new(db)
	}

	#[test]
	fn loader_from_db() {
		let db = test_db();
		let loader = LanguageLoader::from_db(db);

		let lang = loader.language_for_name("rust").unwrap();
		assert_eq!(lang.idx(), 0);
		assert_eq!(loader.language_for_path(Path::new("test.rs")), Some(lang));
	}

	#[test]
	fn shebang_detection() {
		let db = test_db();
		let loader = LanguageLoader::from_db(db);

		let lang = loader.language_for_name("python").unwrap();

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

	#[test]
	fn from_embedded_uses_global_db() {
		let loader = LanguageLoader::from_embedded();
		assert!(!loader.is_empty());

		let rust = loader.language_for_name("rust").expect("rust language");
		let data = loader.get(rust).unwrap();
		assert!(data.extensions.contains(&"rs".to_string()));
	}
}
