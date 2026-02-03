//! Persistent workspace search index for the broker.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};

use bumpalo::Bump;
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::protocol::value::Value;
use helix_db::utils::properties::ImmutablePropertiesMap;
use ropey::Rope;
use xeno_broker_proto::types::{SyncEpoch, SyncSeq};

/// Project crawler for background indexing.
pub mod crawler;
/// Error types for the knowledge system.
pub mod error;
/// Background indexing worker.
pub mod indexer;
/// Full-text search implementation.
pub mod search;

pub use error::KnowledgeError;

/// Source of authoritative sync document snapshots.
pub trait DocSnapshotSource: Send + Sync + 'static {
	/// Pulls a consistent snapshot of an open document.
	fn snapshot_sync_doc(
		&self,
		uri: &str,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Option<(SyncEpoch, SyncSeq, Rope)>> + Send>,
	>;
	/// Checks if a document is currently open in the editor.
	fn is_sync_doc_open(
		&self,
		uri: &str,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>;
}

/// KnowledgeCore wraps helix-db storage for persistent workspace search.
pub struct KnowledgeCore {
	storage: Arc<HelixGraphStorage>,
	db_path: PathBuf,
	worker: OnceLock<indexer::IndexWorker>,
}

impl KnowledgeCore {
	/// Opens (or creates) the knowledge database at the given path.
	///
	/// # Errors
	///
	/// Returns `KnowledgeError` if the database cannot be initialized.
	pub fn open(db_path: PathBuf) -> Result<Self, KnowledgeError> {
		let storage = crate::core::db::BrokerDb::open(db_path.clone())?.storage();
		Ok(Self::from_storage(storage, db_path))
	}

	/// Builds a knowledge core from an existing helix-db storage handle.
	pub fn from_storage(storage: Arc<HelixGraphStorage>, db_path: PathBuf) -> Self {
		Self {
			storage,
			db_path,
			worker: OnceLock::new(),
		}
	}

	/// Returns the underlying helix-db storage handle.
	pub fn storage(&self) -> &Arc<HelixGraphStorage> {
		&self.storage
	}

	/// Starts the background indexing worker if a Tokio runtime is available.
	pub fn start_worker(&self, source: Weak<dyn DocSnapshotSource>) {
		if self.worker.get().is_some() {
			return;
		}
		if tokio::runtime::Handle::try_current().is_err() {
			return;
		}

		let worker = indexer::IndexWorker::spawn(self.storage().clone(), source);
		let _ = self.worker.set(worker);
	}

	/// Marks a URI as dirty for background indexing.
	pub fn mark_dirty(&self, uri: String) {
		if let Some(worker) = self.worker.get() {
			worker.mark_dirty(uri);
		}
	}

	/// Enqueues a file for background indexing.
	pub async fn enqueue_file(&self, uri: String, rope: Rope, language: String, mtime: u64) {
		if let Some(worker) = self.worker.get() {
			worker.enqueue_file(uri, rope, language, mtime).await;
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
