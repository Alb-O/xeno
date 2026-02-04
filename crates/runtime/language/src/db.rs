//! Language database: single source of truth for language configuration.
//!
//! The [`LanguageDb`] consolidates all language metadata into a single structure,
//! parsed once from `languages.kdl`. Lookups are backed by simple HashMaps for
//! fast access by name, extension, filename, and shebang.

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
/// Holds all registered language metadata with in-memory indices for
/// fast lookups by name, extension, filename, and shebang.
pub struct LanguageDb {
	languages: Vec<LanguageData>,
	globs: Vec<(String, usize)>,
	/// Map from language name to index.
	by_name: HashMap<String, usize>,
	/// Map from extension to language index (first registered wins).
	by_extension: HashMap<String, usize>,
	/// Map from filename to language index.
	by_filename: HashMap<String, usize>,
	/// Map from shebang to language index.
	by_shebang: HashMap<String, usize>,
}

impl LanguageDb {
	/// Creates an empty database.
	pub fn new() -> Self {
		Self {
			languages: Vec::new(),
			globs: Vec::new(),
			by_name: HashMap::new(),
			by_extension: HashMap::new(),
			by_filename: HashMap::new(),
			by_shebang: HashMap::new(),
		}
	}

	/// Creates a database populated from the embedded `languages.kdl`.
	pub fn from_embedded() -> Self {
		let mut db = Self::new();
		match load_language_configs() {
			Ok(langs) => db.register_all(langs),
			Err(e) => error!(error = %e, "Failed to load language configs"),
		}
		db
	}

	/// Registers a language and adds it to all indices.
	///
	/// Returns the language ID for the registered language.
	pub fn register(&mut self, data: LanguageData) -> Language {
		let idx = self.languages.len() as u32;
		self.register_all(vec![data]);
		Language::new(idx)
	}

	/// Batch-registers languages.
	fn register_all(&mut self, languages: Vec<LanguageData>) {
		let start_idx = self.languages.len();

		for (i, data) in languages.iter().enumerate() {
			let idx = start_idx + i;

			// Index by name.
			self.by_name.entry(data.name.clone()).or_insert(idx);

			// Index by extension (first registered wins).
			for ext in &data.extensions {
				self.by_extension.entry(ext.clone()).or_insert(idx);
			}

			// Index by filename.
			for fname in &data.filenames {
				self.by_filename.entry(fname.clone()).or_insert(idx);
			}

			// Index by shebang.
			for shebang in &data.shebangs {
				self.by_shebang.entry(shebang.clone()).or_insert(idx);
			}

			// Collect globs.
			for glob in &data.globs {
				self.globs.push((glob.clone(), idx));
			}
		}

		self.languages.extend(languages);
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
		let idx = self.index_for_name(language)?;
		let lang = &self.languages[idx];
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

impl Default for LanguageDb {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for LanguageDb {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LanguageDb")
			.field("languages", &self.languages.len())
			.field("globs", &self.globs.len())
			.finish_non_exhaustive()
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

	#[test]
	fn collision_returns_lower_idx() {
		let mut db = LanguageDb::new();
		db.register(LanguageData::new(
			"cpp".to_string(),
			None,
			vec!["h".to_string(), "cpp".to_string()],
			vec![],
			vec![],
			vec![],
			vec![],
			None,
			None,
			vec![],
			vec![],
		));
		db.register(LanguageData::new(
			"c".to_string(),
			None,
			vec!["h".to_string(), "c".to_string()],
			vec![],
			vec![],
			vec![],
			vec![],
			None,
			None,
			vec![],
			vec![],
		));

		let idx = db.index_for_extension("h").expect("h extension");
		assert_eq!(
			idx, 0,
			"shared extension should resolve to first registered"
		);
	}

	#[test]
	fn multi_key_indexing() {
		let mut db = LanguageDb::new();
		db.register(LanguageData::new(
			"rust".to_string(),
			None,
			vec!["rs".to_string()],
			vec!["Cargo.toml".to_string()],
			vec![],
			vec!["rust-script".to_string(), "cargo".to_string()],
			vec![],
			None,
			None,
			vec![],
			vec![],
		));

		assert!(db.index_for_extension("rs").is_some());
		assert!(db.index_for_filename("Cargo.toml").is_some());
		assert!(db.index_for_shebang("rust-script").is_some());
		assert!(db.index_for_shebang("cargo").is_some());
		assert!(db.index_for_extension("py").is_none());
	}
}
