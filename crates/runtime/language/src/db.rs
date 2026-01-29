//! Language database: single source of truth for language configuration.
//!
//! The [`LanguageDb`] consolidates all language metadata into a single structure,
//! parsed once from `languages.kdl`. This eliminates duplicate parsing that
//! previously occurred in separate config and LSP mapping modules.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use tracing::error;
use tree_house::Language;

use crate::config::load_language_configs;
use crate::language::LanguageData;
use crate::lsp_config::LanguageLspInfo;

/// Global language database, initialized on first access.
static LANG_DB: OnceLock<Arc<LanguageDb>> = OnceLock::new();

/// Returns the global language database, initializing it if needed.
pub fn language_db() -> &'static Arc<LanguageDb> {
	LANG_DB.get_or_init(|| Arc::new(LanguageDb::from_embedded()))
}

/// Consolidated language configuration database.
///
/// Holds all registered language metadata with lookup indices for fast access.
/// Parsed once from `languages.kdl` and accessed via [`language_db()`].
#[derive(Debug, Default)]
pub struct LanguageDb {
	languages: Vec<LanguageData>,
	by_extension: HashMap<String, usize>,
	by_filename: HashMap<String, usize>,
	globs: Vec<(String, usize)>,
	by_shebang: HashMap<String, usize>,
	by_name: HashMap<String, usize>,
}

impl LanguageDb {
	/// Creates an empty database.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a database populated from the embedded `languages.kdl`.
	pub fn from_embedded() -> Self {
		let mut db = Self::new();
		match load_language_configs() {
			Ok(langs) => {
				for lang in langs {
					db.register(lang);
				}
			}
			Err(e) => error!(error = %e, "Failed to load language configs"),
		}
		db
	}

	/// Registers a language and builds lookup indices.
	///
	/// Returns the language ID for the registered language.
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

	/// Returns language data by index.
	pub fn get(&self, idx: usize) -> Option<&LanguageData> {
		self.languages.get(idx)
	}

	/// Returns the index for a language name.
	pub fn index_for_name(&self, name: &str) -> Option<usize> {
		self.by_name.get(name).copied()
	}

	/// Returns the index for a file extension.
	pub fn index_for_extension(&self, ext: &str) -> Option<usize> {
		self.by_extension.get(ext).copied()
	}

	/// Returns the index for an exact filename.
	pub fn index_for_filename(&self, filename: &str) -> Option<usize> {
		self.by_filename.get(filename).copied()
	}

	/// Returns the index for a shebang interpreter.
	pub fn index_for_shebang(&self, interpreter: &str) -> Option<usize> {
		self.by_shebang.get(interpreter).copied()
	}

	/// Returns glob patterns with their language indices.
	pub fn globs(&self) -> &[(String, usize)] {
		&self.globs
	}

	/// Returns all registered languages.
	pub fn languages(&self) -> impl Iterator<Item = (usize, &LanguageData)> {
		self.languages.iter().enumerate()
	}

	/// Returns the number of registered languages.
	pub fn len(&self) -> usize {
		self.languages.len()
	}

	/// Returns true if no languages are registered.
	pub fn is_empty(&self) -> bool {
		self.languages.is_empty()
	}

	/// Returns LSP configuration for a language.
	///
	/// Returns `None` if the language has no LSP servers configured.
	pub fn lsp_info(&self, language: &str) -> Option<LanguageLspInfo> {
		let idx = self.by_name.get(language)?;
		let lang = &self.languages[*idx];
		if lang.lsp_servers.is_empty() {
			return None;
		}
		Some(LanguageLspInfo {
			servers: lang.lsp_servers.clone(),
			roots: lang.roots.clone(),
		})
	}

	/// Returns a mapping of all languages to their LSP info.
	///
	/// Only includes languages that have LSP servers configured.
	pub fn lsp_mapping(&self) -> HashMap<String, LanguageLspInfo> {
		self.languages
			.iter()
			.filter(|lang| !lang.lsp_servers.is_empty())
			.map(|lang| {
				(
					lang.name.clone(),
					LanguageLspInfo {
						servers: lang.lsp_servers.clone(),
						roots: lang.roots.clone(),
					},
				)
			})
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn db_from_embedded() {
		let db = LanguageDb::from_embedded();
		assert!(!db.is_empty());

		let rust_idx = db.index_for_name("rust").expect("rust language");
		let rust = db.get(rust_idx).unwrap();
		assert!(rust.extensions.contains(&"rs".to_string()));
	}

	#[test]
	fn lsp_info_returns_configured_servers() {
		let db = LanguageDb::from_embedded();
		let info = db.lsp_info("rust").expect("rust has LSP");
		assert!(info.servers.contains(&"rust-analyzer".to_string()));
	}

	#[test]
	fn lsp_mapping_excludes_languages_without_servers() {
		let db = LanguageDb::from_embedded();
		let mapping = db.lsp_mapping();

		assert!(mapping.contains_key("rust"));
		for info in mapping.values() {
			assert!(!info.servers.is_empty());
		}
	}
}
