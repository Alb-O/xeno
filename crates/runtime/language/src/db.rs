//! Language database: single source of truth for language configuration.
//!
//! The [`LanguageDb`] consolidates all language metadata into a single structure,
//! parsed once from `languages.kdl`. Lookups are backed by helix-db secondary
//! indices; runtime data (syntax configs, LSP info) is served from an in-memory
//! `Vec<LanguageData>`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, OnceLock};

use bumpalo::Bump;
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::helix_engine::storage_core::version_info::VersionInfo;
use helix_db::helix_engine::traversal_core::config::{Config, GraphConfig};
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::source::add_n::AddNAdapter;
use helix_db::helix_engine::traversal_core::ops::source::n_from_index::NFromIndexAdapter;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::helixc::analyzer::analyze;
use helix_db::helixc::analyzer::diagnostic::DiagnosticSeverity;
use helix_db::helixc::parser::HelixParser;
use helix_db::helixc::parser::types::{Content, HxFile, Source as ParsedSource};
use helix_db::protocol::value::Value;
use helix_db::utils::properties::ImmutablePropertiesMap;
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
/// Holds all registered language metadata with helix-db secondary indices for
/// fast lookups by name, extension, filename, and shebang. Runtime data
/// (syntax configs, LSP info) is served from the in-memory `languages` vec.
pub struct LanguageDb {
	languages: Vec<LanguageData>,
	globs: Vec<(String, usize)>,
	storage: Arc<HelixGraphStorage>,
	_db_dir: tempfile::TempDir,
}

const SCHEMA_HQL: &str = include_str!("../schema.hql");

/// Node label derived from the HQL schema (`N::Language`).
const LABEL: &str = "Language";

/// Helix-db config derived from `schema.hql` at first access.
///
/// Parses the embedded HQL schema, runs the analyzer to extract secondary
/// index declarations, and builds the storage config.
static SCHEMA_CONFIG: LazyLock<Config> = LazyLock::new(|| {
	let content = Content {
		content: String::new(),
		source: ParsedSource::default(),
		files: vec![HxFile {
			name: "schema.hql".into(),
			content: SCHEMA_HQL.into(),
		}],
	};
	let parsed = HelixParser::parse_source(&content).expect("schema.hql: parse failed");
	let (diags, generated) = analyze(&parsed).expect("schema.hql: analysis failed");

	for d in &diags {
		if matches!(d.severity, DiagnosticSeverity::Error) {
			panic!("schema.hql: {d:?}");
		}
	}

	Config {
		graph_config: Some(GraphConfig {
			secondary_indices: if generated.secondary_indices.is_empty() {
				None
			} else {
				Some(generated.secondary_indices)
			},
		}),
		db_max_size_gb: Some(1),
		mcp: Some(false),
		bm25: Some(false),
		..Config::default()
	}
});

impl LanguageDb {
	/// Creates an empty database backed by a temporary directory.
	pub fn new() -> Self {
		let db_dir = tempfile::tempdir().expect("failed to create tempdir for LanguageDb");
		let storage = HelixGraphStorage::new(
			db_dir.path().to_str().unwrap_or("lang_db"),
			SCHEMA_CONFIG.clone(),
			VersionInfo::default(),
		)
		.expect("failed to open helix-db storage");

		Self {
			languages: Vec::new(),
			globs: Vec::new(),
			storage: Arc::new(storage),
			_db_dir: db_dir,
		}
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

	/// Registers a language and writes it to helix-db indices.
	///
	/// Returns the language ID for the registered language.
	pub fn register(&mut self, data: LanguageData) -> Language {
		let idx = self.languages.len();
		let arena = Bump::new();

		// Build properties for the helix-db node.
		let entries: Vec<(&str, Value)> = vec![
			("name", Value::String(data.name.clone())),
			("idx", Value::U32(idx as u32)),
		];

		let prop_count = entries.len();
		let props = ImmutablePropertiesMap::new(
			prop_count,
			entries.into_iter().map(|(k, v)| {
				let k: &str = arena.alloc_str(k);
				(k, v)
			}),
			&arena,
		);

		let mut txn = self
			.storage
			.graph_env
			.write_txn()
			.expect("write txn for register");

		let result = G::new_mut(&self.storage, &arena, &mut txn)
			.add_n(arena.alloc_str(LABEL), Some(props), Some(&["name"]))
			.next()
			.unwrap()
			.expect("add_n failed");

		let node_id = result.id();

		// Index each extension into the extension_idx secondary index.
		for ext in &data.extensions {
			let key = postcard::to_stdvec(&Value::String(ext.clone()))
				.expect("postcard serialize extension");
			let (db, active) = self.storage.secondary_indices.get("extension_idx").unwrap();
			active
				.insert(db, &mut txn, &key, &node_id)
				.expect("insert extension_idx");
		}

		// Index each filename into the filename_idx secondary index.
		for fname in &data.filenames {
			let key = postcard::to_stdvec(&Value::String(fname.clone()))
				.expect("postcard serialize filename");
			let (db, active) = self.storage.secondary_indices.get("filename_idx").unwrap();
			active
				.insert(db, &mut txn, &key, &node_id)
				.expect("insert filename_idx");
		}

		// Index each shebang into the shebang_idx secondary index.
		for shebang in &data.shebangs {
			let key = postcard::to_stdvec(&Value::String(shebang.clone()))
				.expect("postcard serialize shebang");
			let (db, active) = self.storage.secondary_indices.get("shebang_idx").unwrap();
			active
				.insert(db, &mut txn, &key, &node_id)
				.expect("insert shebang_idx");
		}

		txn.commit().expect("commit register txn");

		// Maintain in-memory collections.
		for glob in &data.globs {
			self.globs.push((glob.clone(), idx));
		}
		self.languages.push(data);

		Language::new(idx as u32)
	}

	/// Returns language data by index.
	pub fn get(&self, idx: usize) -> Option<&LanguageData> {
		self.languages.get(idx)
	}

	/// Returns the index for a language name.
	pub fn index_for_name(&self, name: &str) -> Option<usize> {
		self.lookup_idx("name", name)
	}

	/// Returns the index for a file extension.
	pub fn index_for_extension(&self, ext: &str) -> Option<usize> {
		self.lookup_idx("extension_idx", ext)
	}

	/// Returns the index for an exact filename.
	pub fn index_for_filename(&self, filename: &str) -> Option<usize> {
		self.lookup_idx("filename_idx", filename)
	}

	/// Returns the index for a shebang interpreter.
	pub fn index_for_shebang(&self, interpreter: &str) -> Option<usize> {
		self.lookup_idx("shebang_idx", interpreter)
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

	/// Queries a secondary index and extracts the `idx` property from the first match.
	fn lookup_idx(&self, index: &str, key: &str) -> Option<usize> {
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn().ok()?;

		let node = G::new(&self.storage, &txn, &arena)
			.n_from_index(LABEL, index, &key)
			.filter_map(|r| r.ok())
			.next()?;

		if let TraversalValue::Node(n) = node {
			match n.get_property("idx") {
				Some(Value::U32(i)) => Some(*i as usize),
				_ => None,
			}
		} else {
			None
		}
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
}
