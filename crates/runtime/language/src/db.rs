//! Language database: single source of truth for language configuration.
//!
//! The [`LanguageDb`] consolidates all language metadata into a single structure,
//! parsed once from `languages.kdl`. Lookups are backed by helix-db secondary
//! indices on separate mapping node types (`LangExtension`, `LangFilename`,
//! `LangShebang`), each carrying the language's positional index. Runtime data
//! (syntax configs, LSP info) is served from an in-memory `Vec<LanguageData>`.

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
/// fast lookups by name, extension, filename, and shebang. Multi-value keys
/// (extensions, filenames, shebangs) are modeled as separate mapping node types
/// (`LangExtension`, `LangFilename`, `LangShebang`), each carrying the
/// language's positional `idx`. Runtime data (syntax configs, LSP info) is
/// served from the in-memory `languages` vec.
pub struct LanguageDb {
	languages: Vec<LanguageData>,
	globs: Vec<(String, usize)>,
	storage: Arc<HelixGraphStorage>,
	_db_dir: tempfile::TempDir,
}

const SCHEMA_HQL: &str = include_str!("../schema.hql");

/// Node labels derived from the HQL schema.
const LABEL_LANGUAGE: &str = "Language";
const LABEL_EXTENSION: &str = "LangExtension";
const LABEL_FILENAME: &str = "LangFilename";
const LABEL_SHEBANG: &str = "LangShebang";

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

/// Builds an [`ImmutablePropertiesMap`] from key-value entries, allocating
/// keys into the provided arena.
fn build_props<'arena>(
	arena: &'arena Bump,
	entries: Vec<(&str, Value)>,
) -> ImmutablePropertiesMap<'arena> {
	let prop_count = entries.len();
	ImmutablePropertiesMap::new(
		prop_count,
		entries.into_iter().map(|(k, v)| {
			let k: &str = arena.alloc_str(k);
			(k, v)
		}),
		arena,
	)
}

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
			Ok(langs) => db.register_all(langs),
			Err(e) => error!(error = %e, "Failed to load language configs"),
		}
		db
	}

	/// Registers a language and writes it + mapping nodes to helix-db.
	///
	/// Returns the language ID for the registered language.
	pub fn register(&mut self, data: LanguageData) -> Language {
		let idx = self.languages.len() as u32;
		self.register_all(vec![data]);
		Language::new(idx)
	}

	/// Batch-registers languages in a single transaction.
	///
	/// Inserts a `Language` node plus `LangExtension`, `LangFilename`, and
	/// `LangShebang` mapping nodes for each language. All writes share one
	/// LMDB write transaction for atomicity and performance.
	fn register_all(&mut self, languages: Vec<LanguageData>) {
		let start_idx = self.languages.len();
		let arena = Bump::new();
		let mut txn = self
			.storage
			.graph_env
			.write_txn()
			.expect("write txn for register_all");

		for (i, data) in languages.iter().enumerate() {
			let idx = (start_idx + i) as u32;

			// Language node.
			let props = build_props(
				&arena,
				vec![
					("name", Value::String(data.name.clone())),
					("idx", Value::U32(idx)),
				],
			);
			G::new_mut(&self.storage, &arena, &mut txn)
				.add_n(
					arena.alloc_str(LABEL_LANGUAGE),
					Some(props),
					Some(&["name"]),
				)
				.next()
				.unwrap()
				.expect("add_n Language failed");

			// LangExtension mapping nodes.
			for ext in &data.extensions {
				let props = build_props(
					&arena,
					vec![
						("extension", Value::String(ext.clone())),
						("idx", Value::U32(idx)),
					],
				);
				G::new_mut(&self.storage, &arena, &mut txn)
					.add_n(
						arena.alloc_str(LABEL_EXTENSION),
						Some(props),
						Some(&["extension"]),
					)
					.next()
					.unwrap()
					.expect("add_n LangExtension failed");
			}

			// LangFilename mapping nodes.
			for fname in &data.filenames {
				let props = build_props(
					&arena,
					vec![
						("filename", Value::String(fname.clone())),
						("idx", Value::U32(idx)),
					],
				);
				G::new_mut(&self.storage, &arena, &mut txn)
					.add_n(
						arena.alloc_str(LABEL_FILENAME),
						Some(props),
						Some(&["filename"]),
					)
					.next()
					.unwrap()
					.expect("add_n LangFilename failed");
			}

			// LangShebang mapping nodes.
			for shebang in &data.shebangs {
				let props = build_props(
					&arena,
					vec![
						("shebang", Value::String(shebang.clone())),
						("idx", Value::U32(idx)),
					],
				);
				G::new_mut(&self.storage, &arena, &mut txn)
					.add_n(
						arena.alloc_str(LABEL_SHEBANG),
						Some(props),
						Some(&["shebang"]),
					)
					.next()
					.unwrap()
					.expect("add_n LangShebang failed");
			}
		}

		txn.commit().expect("commit register_all txn");

		for (i, data) in languages.iter().enumerate() {
			for glob in &data.globs {
				self.globs.push((glob.clone(), start_idx + i));
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
		self.lookup_min_idx(LABEL_LANGUAGE, "name", name)
	}

	/// Returns the index for a file extension.
	pub fn index_for_extension(&self, ext: &str) -> Option<usize> {
		self.lookup_min_idx(LABEL_EXTENSION, "extension", ext)
	}

	/// Returns the index for an exact filename.
	pub fn index_for_filename(&self, filename: &str) -> Option<usize> {
		self.lookup_min_idx(LABEL_FILENAME, "filename", filename)
	}

	/// Returns the index for a shebang interpreter.
	pub fn index_for_shebang(&self, interpreter: &str) -> Option<usize> {
		self.lookup_min_idx(LABEL_SHEBANG, "shebang", interpreter)
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

	/// Queries a secondary index and returns the minimum `idx` across all
	/// matching nodes, giving deterministic "first registered wins" semantics.
	fn lookup_min_idx(&self, label: &str, index: &str, key: &str) -> Option<usize> {
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn().ok()?;

		G::new(&self.storage, &txn, &arena)
			.n_from_index(label, index, &key)
			.filter_map(|r| r.ok())
			.filter_map(|tv| {
				if let TraversalValue::Node(n) = tv {
					match n.get_property("idx") {
						Some(Value::U32(i)) => Some(*i as usize),
						_ => None,
					}
				} else {
					None
				}
			})
			.min()
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
