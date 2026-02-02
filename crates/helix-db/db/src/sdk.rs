/// Embedded SDK for HelixDB.
///
/// Provides a high-level entry point (`HelixDB`) that wraps the engine and
/// exposes the raw storage handle for direct use with the `G` traversal API.
///
/// # Usage
///
/// ```rust,no_run
/// use helix_db::sdk::{HelixDB, HelixDBClient};
/// use helix_db::helix_engine::traversal_core::ops::g::G;
///
/// let db = HelixDB::new("/tmp/mydb").unwrap();
/// let storage = db.storage();
/// let arena = bumpalo::Bump::new();
/// let txn = storage.graph_env.read_txn().unwrap();
/// let traversal = G::new(&storage, &txn, &arena);
/// ```
use std::sync::Arc;

use crate::helix_engine::storage_core::version_info::VersionInfo;
use crate::helix_engine::storage_core::HelixGraphStorage;
use crate::helix_engine::traversal_core::config::Config;
use crate::helix_engine::traversal_core::HelixGraphEngine;
use crate::helix_engine::traversal_core::HelixGraphEngineOpts;
use crate::helix_engine::types::EngineError;

pub use helix_macros::helix_node;

/// Client trait for embedded HelixDB access.
pub trait HelixDBClient {
	type Err: std::error::Error;

	fn new(path: &str) -> Result<Self, Self::Err>
	where
		Self: Sized;

	fn storage(&self) -> &Arc<HelixGraphStorage>;
}

/// Embedded HelixDB client wrapping [`HelixGraphEngine`].
pub struct HelixDB {
	engine: HelixGraphEngine,
}

/// Errors returned by the embedded SDK.
#[derive(Debug, thiserror::Error)]
pub enum HelixError {
	#[error(transparent)]
	Engine(#[from] EngineError),
}

impl HelixDB {
	/// Opens or creates a database at `path` with the given configuration.
	pub fn with_config(path: &str, config: Config) -> Result<Self, HelixError> {
		let engine = HelixGraphEngine::new(HelixGraphEngineOpts {
			path: path.to_string(),
			config,
			version_info: VersionInfo::default(),
		})?;
		Ok(Self { engine })
	}
}

impl HelixDBClient for HelixDB {
	type Err = HelixError;

	/// Opens or creates a database at `path` with default configuration.
	fn new(path: &str) -> Result<Self, HelixError> {
		Self::with_config(path, Config::default())
	}

	fn storage(&self) -> &Arc<HelixGraphStorage> {
		&self.engine.storage
	}
}
