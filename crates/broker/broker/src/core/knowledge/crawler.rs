use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::UNIX_EPOCH;

use bumpalo::Bump;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::source::n_from_index::NFromIndexAdapter;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::protocol::value::Value;
use ignore::WalkBuilder;
use xeno_runtime_language::{LanguageDb, language_db};

use super::indexer::index_document;
use super::{DocSnapshotSource, KnowledgeCore, KnowledgeError};

const LABEL_DOC: &str = "Doc";
const INDEX_DOC_URI: &str = "uri";

/// Background project crawler for indexing unopened files.
pub struct ProjectCrawler;

impl ProjectCrawler {
	pub fn spawn(
		knowledge: Arc<KnowledgeCore>,
		source: Weak<dyn DocSnapshotSource>,
		root: PathBuf,
	) -> Option<Self> {
		if tokio::runtime::Handle::try_current().is_err() {
			return None;
		}

		let crawler = Self;
		tokio::spawn(async move {
			let _ = tokio::task::spawn_blocking(move || {
				crawl_project(knowledge, source, root);
			})
			.await;
		});

		Some(crawler)
	}
}

fn crawl_project(
	knowledge: Arc<KnowledgeCore>,
	source: Weak<dyn DocSnapshotSource>,
	root: PathBuf,
) {
	let lang_db = language_db();
	let walker = WalkBuilder::new(&root)
		.standard_filters(true)
		.follow_links(false)
		.build();

	for entry in walker {
		let entry = match entry {
			Ok(entry) => entry,
			Err(err) => {
				tracing::warn!(error = %err, "crawler entry error");
				continue;
			}
		};

		let Some(file_type) = entry.file_type() else {
			continue;
		};
		if !file_type.is_file() {
			continue;
		}

		let path = entry.into_path();
		if !is_indexable_path(lang_db, &path) {
			continue;
		}

		let Some(uri) = xeno_lsp::uri_from_path(&path).map(|uri| uri.to_string()) else {
			continue;
		};

		let Some(source) = source.upgrade() else {
			break;
		};
		if source.is_sync_doc_open(&uri) {
			continue;
		}

		let Some(mtime) = file_mtime(&path) else {
			continue;
		};

		match doc_mtime_matches(knowledge.storage(), &uri, mtime) {
			Ok(true) => continue,
			Ok(false) => {}
			Err(err) => {
				tracing::warn!(error = %err, ?uri, "crawler metadata lookup failed");
				continue;
			}
		}

		let text = match std::fs::read_to_string(&path) {
			Ok(text) => text,
			Err(err) => {
				tracing::warn!(error = %err, path = %path.display(), "crawler read failed");
				continue;
			}
		};

		let language = language_name_for_path(lang_db, &path).unwrap_or_default();
		if let Err(err) = index_document(
			knowledge.storage(),
			&uri,
			&text,
			0,
			0,
			&language,
			Some(mtime),
		) {
			tracing::warn!(error = %err, ?uri, "crawler index failed");
		}

		std::thread::yield_now();
	}
}

fn file_mtime(path: &Path) -> Option<u64> {
	let metadata = std::fs::metadata(path).ok()?;
	let modified = metadata.modified().ok()?;
	let dur = modified.duration_since(UNIX_EPOCH).ok()?;
	Some(dur.as_secs())
}

fn is_indexable_path(db: &LanguageDb, path: &Path) -> bool {
	let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
		return false;
	};
	db.index_for_extension(ext).is_some()
}

fn language_name_for_path(db: &LanguageDb, path: &Path) -> Option<String> {
	let ext = path.extension()?.to_str()?;
	let idx = db.index_for_extension(ext)?;
	db.languages()
		.find_map(|(i, data)| (i == idx).then(|| data.name.clone()))
}

fn doc_mtime_matches(
	storage: &helix_db::helix_engine::storage_core::HelixGraphStorage,
	uri: &str,
	mtime: u64,
) -> Result<bool, KnowledgeError> {
	let arena = Bump::new();
	let txn = storage
		.graph_env
		.read_txn()
		.map_err(helix_db::helix_engine::types::EngineError::from)?;

	for entry in G::new(storage, &txn, &arena).n_from_index(LABEL_DOC, INDEX_DOC_URI, &uri) {
		if let Ok(TraversalValue::Node(node)) = entry {
			if let Some(Value::U64(value)) = node.get_property("mtime") {
				return Ok(*value != 0 && *value == mtime);
			}
			return Ok(false);
		}
	}

	Ok(false)
}

#[cfg(test)]
mod tests {
	use std::collections::HashSet;
	use std::sync::Arc;

	use ropey::Rope;
	use tempfile::TempDir;
	use xeno_broker_proto::types::{SyncEpoch, SyncSeq};

	use super::{crawl_project, doc_mtime_matches, file_mtime};
	use crate::core::knowledge::indexer::index_document;
	use crate::core::knowledge::{DocSnapshotSource, KnowledgeCore};

	struct TestSource {
		open_uris: HashSet<String>,
	}

	impl DocSnapshotSource for TestSource {
		fn snapshot_sync_doc(&self, _uri: &str) -> Option<(SyncEpoch, SyncSeq, Rope)> {
			None
		}

		fn is_sync_doc_open(&self, uri: &str) -> bool {
			self.open_uris.contains(uri)
		}
	}

	#[test]
	fn test_doc_mtime_matches() {
		let temp = TempDir::new().expect("tempdir");
		let core = KnowledgeCore::open(temp.path().join("knowledge")).expect("open knowledge");
		let uri = "file:///mtime.rs";

		index_document(core.storage(), uri, "hello", 1, 1, "", Some(10)).expect("index");
		assert!(doc_mtime_matches(core.storage(), uri, 10).expect("mtime match"));
		assert!(!doc_mtime_matches(core.storage(), uri, 11).expect("mtime mismatch"));

		index_document(core.storage(), uri, "hello", 1, 2, "", None).expect("reindex");
		assert!(doc_mtime_matches(core.storage(), uri, 10).expect("mtime preserved"));
	}

	#[test]
	fn test_index_document_mtime_overridden_when_some() {
		let temp = TempDir::new().expect("tempdir");
		let core = KnowledgeCore::open(temp.path().join("knowledge")).expect("open knowledge");
		let uri = "file:///mtime_override.rs";

		index_document(core.storage(), uri, "hello", 1, 1, "", Some(10)).expect("index");
		assert!(doc_mtime_matches(core.storage(), uri, 10).expect("mtime match"));

		index_document(core.storage(), uri, "hello", 1, 2, "", Some(11)).expect("reindex");
		assert!(doc_mtime_matches(core.storage(), uri, 11).expect("mtime override"));
	}

	#[test]
	fn test_crawler_skips_open_sync_docs() {
		let temp = TempDir::new().expect("tempdir");
		let root = temp.path().join("workspace");
		std::fs::create_dir_all(&root).expect("create root");

		let path = root.join("main.rs");
		std::fs::write(&path, "fn main() {}\n").expect("write file");

		let knowledge =
			Arc::new(KnowledgeCore::open(temp.path().join("knowledge")).expect("open knowledge"));
		let uri = xeno_lsp::uri_from_path(&path)
			.map(|uri| uri.to_string())
			.expect("uri");
		let mtime = file_mtime(&path).expect("mtime");

		let mut open_uris = HashSet::new();
		open_uris.insert(uri.clone());
		let source: Arc<dyn DocSnapshotSource> = Arc::new(TestSource { open_uris });
		let weak = Arc::downgrade(&source);

		crawl_project(Arc::clone(&knowledge), weak, root);

		assert!(!doc_mtime_matches(knowledge.storage(), &uri, mtime).expect("mtime check"));
	}
}
