//! Persistent workspace search index for the broker.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock, Weak};

use bumpalo::Bump;
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::helix_engine::storage_core::version_info::VersionInfo;
use helix_db::helix_engine::traversal_core::config::{Config, GraphConfig};
use helix_db::helixc::analyzer::analyze;
use helix_db::helixc::analyzer::diagnostic::DiagnosticSeverity;
use helix_db::helixc::parser::HelixParser;
use helix_db::helixc::parser::types::{Content, HxFile, Source as ParsedSource};
use helix_db::protocol::value::Value;
use helix_db::utils::properties::ImmutablePropertiesMap;

pub mod error;
pub mod indexer;
pub mod search;

pub use error::KnowledgeError;

const SCHEMA_HQL: &str = include_str!("schema.hql");

/// Helix-db config derived from `schema.hql` at first access.
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
		db_max_size_gb: Some(2),
		mcp: Some(false),
		bm25: Some(true),
		..Config::default()
	}
});

/// Returns the default knowledge DB path under the user state directory.
pub fn default_db_path() -> Result<PathBuf, KnowledgeError> {
	let state_dir = dirs::state_dir()
		.or_else(|| dirs::home_dir().map(|home| home.join(".local/state")))
		.ok_or(KnowledgeError::MissingStateDir)?;
	Ok(state_dir.join("xeno").join("knowledge"))
}

/// KnowledgeCore wraps helix-db storage for persistent workspace search.
pub struct KnowledgeCore {
	storage: Arc<HelixGraphStorage>,
	db_path: PathBuf,
	worker: OnceLock<indexer::IndexWorker>,
}

impl KnowledgeCore {
	/// Opens (or creates) the knowledge database at the given path.
	pub fn open(db_path: PathBuf) -> Result<Self, KnowledgeError> {
		std::fs::create_dir_all(&db_path)?;

		let path_str = db_path.to_str().unwrap_or("knowledge_db");
		let storage =
			HelixGraphStorage::new(path_str, SCHEMA_CONFIG.clone(), VersionInfo::default())?;

		Ok(Self {
			storage: Arc::new(storage),
			db_path,
			worker: OnceLock::new(),
		})
	}

	/// Returns the underlying helix-db storage handle.
	pub fn storage(&self) -> &Arc<HelixGraphStorage> {
		&self.storage
	}

	/// Starts the background indexing worker if a Tokio runtime is available.
	pub fn start_worker(&self, broker: Weak<super::BrokerCore>) {
		if self.worker.get().is_some() {
			return;
		}
		if tokio::runtime::Handle::try_current().is_err() {
			return;
		}

		let worker = indexer::IndexWorker::spawn(self.storage().clone(), broker);
		let _ = self.worker.set(worker);
	}

	/// Marks a URI as dirty for background indexing.
	pub fn mark_dirty(&self, uri: String) {
		if let Some(worker) = self.worker.get() {
			worker.mark_dirty(uri);
		}
	}
}

impl fmt::Debug for KnowledgeCore {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("KnowledgeCore")
			.field("db_path", &self.db_path)
			.finish_non_exhaustive()
	}
}

/// Builds an [`ImmutablePropertiesMap`] from key-value entries.
pub(crate) fn build_props<'arena>(
	arena: &'arena Bump,
	entries: Vec<(&'static str, Value)>,
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

#[cfg(test)]
mod tests;
