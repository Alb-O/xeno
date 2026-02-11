use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use tree_house::LanguageConfig as TreeHouseConfig;
use xeno_registry::db::LANGUAGES;
use xeno_registry::languages::registry::LanguageRef;
use xeno_registry::{DenseId, LanguageId};

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
/// Wraps the registry-backed language index and provides caching for
/// runtime-loaded syntax configurations.
pub struct LanguageDb {
	configs: Vec<OnceLock<Option<Arc<TreeHouseConfig>>>>,
}

impl LanguageDb {
	/// Creates an empty database (unused in registry-backed version).
	pub fn new() -> Self {
		Self::from_embedded()
	}

	/// Creates a database populated from the embedded registry.
	pub fn from_embedded() -> Self {
		let len = LANGUAGES.len();
		let mut configs = Vec::with_capacity(len);
		for _ in 0..len {
			configs.push(OnceLock::new());
		}
		Self { configs }
	}

	/// Returns language data by index.
	pub fn get(&self, idx: usize) -> Option<LanguageData> {
		LANGUAGES
			.get_by_id(LanguageId::from_u32(idx as u32))
			.map(|entry: LanguageRef| LanguageData { entry })
	}

	/// Returns the index for a language name.
	pub fn index_for_name(&self, name: &str) -> Option<usize> {
		LANGUAGES.get(name).map(|r: LanguageRef| r.dense_id().as_u32() as usize)
	}

	/// Returns the index for a file extension.
	pub fn index_for_extension(&self, ext: &str) -> Option<usize> {
		LANGUAGES.language_for_extension(ext).map(|r: LanguageRef| r.dense_id().as_u32() as usize)
	}

	/// Returns the index for an exact filename.
	pub fn index_for_filename(&self, filename: &str) -> Option<usize> {
		LANGUAGES.language_for_filename(filename).map(|r: LanguageRef| r.dense_id().as_u32() as usize)
	}

	/// Returns the index for a shebang interpreter.
	pub fn index_for_shebang(&self, interpreter: &str) -> Option<usize> {
		LANGUAGES.language_for_shebang(interpreter).map(|r: LanguageRef| r.dense_id().as_u32() as usize)
	}

	/// Returns glob patterns with their language indices.
	pub fn globs(&self) -> Vec<(String, usize)> {
		(*LANGUAGES)
			.globs()
			.into_iter()
			.map(|(pattern, id): (String, LanguageId)| (pattern, id.as_u32() as usize))
			.collect()
	}

	/// Returns all registered languages.
	pub fn languages(&self) -> impl Iterator<Item = (usize, LanguageData)> {
		LANGUAGES
			.snapshot_guard()
			.iter_refs()
			.map(|entry: LanguageRef| (entry.dense_id().as_u32() as usize, LanguageData { entry }))
	}

	/// Returns the number of registered languages.
	pub fn len(&self) -> usize {
		LANGUAGES.len()
	}

	/// Returns true if no languages are registered.
	pub fn is_empty(&self) -> bool {
		LANGUAGES.is_empty()
	}

	/// Returns the syntax configuration for a language ID.
	pub fn get_config(&self, id: LanguageId) -> Option<&TreeHouseConfig> {
		let lock = self.configs.get(id.as_u32() as usize)?;
		lock.get_or_init(|| {
			let entry = LANGUAGES.get_by_id(id)?;
			crate::language::load_syntax_config(&entry).map(Arc::new)
		})
		.as_ref()
		.map(|arc: &Arc<TreeHouseConfig>| arc.as_ref())
	}

	/// Returns LSP configuration for a language.
	pub fn lsp_info(&self, language: &str) -> Option<LanguageLspInfo> {
		let entry = LANGUAGES.get(language)?;
		if entry.lsp_servers.is_empty() {
			return None;
		}
		Some(LanguageLspInfo {
			servers: entry.lsp_servers.iter().map(|&s| entry.resolve(s).to_string()).collect(),
			roots: entry.roots.iter().map(|&s| entry.resolve(s).to_string()).collect(),
		})
	}

	/// Returns a mapping of all languages to their LSP info.
	pub fn lsp_mapping(&self) -> HashMap<String, LanguageLspInfo> {
		LANGUAGES
			.snapshot_guard()
			.iter_refs()
			.filter(|l: &LanguageRef| !l.lsp_servers.is_empty())
			.map(|l: LanguageRef| {
				(
					l.name_str().to_string(),
					LanguageLspInfo {
						servers: l.lsp_servers.iter().map(|&s| l.resolve(s).to_string()).collect(),
						roots: l.roots.iter().map(|&s| l.resolve(s).to_string()).collect(),
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
		f.debug_struct("LanguageDb").field("languages", &LANGUAGES.len()).finish_non_exhaustive()
	}
}
