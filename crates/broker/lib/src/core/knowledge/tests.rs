use std::sync::{Arc, Mutex};

use bumpalo::Bump;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::source::n_from_index::NFromIndexAdapter;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::protocol::value::Value;
use ropey::Rope;
use tempfile::TempDir;
use tokio::sync::mpsc;
use xeno_broker_proto::types::{ErrorCode, SyncEpoch, SyncSeq};

use super::indexer::{IndexWorker, chunk_text, index_document};
use super::{DocSnapshotSource, KnowledgeCore};
use crate::core::db;
use crate::services::{knowledge, shared_state};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct NullSource;

impl DocSnapshotSource for NullSource {
	fn snapshot_sync_doc(
		&self,
		_uri: &str,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Option<(SyncEpoch, SyncSeq, Rope)>> + Send>,
	> {
		Box::pin(async { None })
	}

	fn is_sync_doc_open(
		&self,
		_uri: &str,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
		Box::pin(async { false })
	}
}

#[test]
fn test_knowledge_core_open_close() {
	let temp = TempDir::new().expect("tempdir");
	let db_path = temp.path().join("knowledge");
	let core = KnowledgeCore::open(db_path).expect("open knowledge core");
	let txn = core.storage().graph_env.read_txn().expect("read txn");
	drop(txn);
}

#[test]
fn test_schema_config_parses() {
	let result = std::panic::catch_unwind(|| {
		let _ = db::schema_config();
	});
	assert!(result.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn test_graceful_degradation() {
	let _guard = ENV_LOCK.lock().unwrap();
	let temp = TempDir::new().expect("tempdir");
	let bad_path = temp.path().join("state-file");
	std::fs::write(&bad_path, "not a directory").expect("write state-file");

	let old_state = std::env::var("XDG_STATE_HOME").ok();
	unsafe {
		std::env::set_var("XDG_STATE_HOME", &bad_path);
	}

	let (sync_tx, _sync_rx) = mpsc::channel(1);
	let sync_handle = shared_state::SharedStateHandle::new(sync_tx);
	let open_docs = Arc::new(Mutex::new(std::collections::HashSet::new()));
	let handle = knowledge::KnowledgeService::start(sync_handle, open_docs, None, None);
	let res = handle.search("missing", 3).await;
	assert_eq!(res.unwrap_err(), ErrorCode::NotImplemented);

	match old_state {
		Some(value) => unsafe {
			std::env::set_var("XDG_STATE_HOME", value);
		},
		None => unsafe {
			std::env::remove_var("XDG_STATE_HOME");
		},
	}
}

fn open_test_core() -> (TempDir, KnowledgeCore) {
	let temp = TempDir::new().expect("tempdir");
	let db_path = temp.path().join("knowledge");
	let core = KnowledgeCore::open(db_path).expect("open knowledge core");
	(temp, core)
}

#[derive(Debug)]
struct DocValues {
	uri: String,
	epoch: u64,
	seq: u64,
	len_chars: u64,
	language: String,
}

fn read_doc_values(
	storage: &helix_db::helix_engine::storage_core::HelixGraphStorage,
	uri: &str,
) -> Option<DocValues> {
	let arena = Bump::new();
	let txn = storage
		.graph_env
		.read_txn()
		.map_err(helix_db::helix_engine::types::EngineError::from)
		.ok()?;
	let doc = G::new(storage, &txn, &arena)
		.n_from_index("Doc", "uri", &uri)
		.filter_map(|entry| entry.ok())
		.find_map(|tv| match tv {
			TraversalValue::Node(node) => Some(node),
			_ => None,
		})?;

	let uri = match doc.get_property("uri") {
		Some(Value::String(value)) => value.clone(),
		_ => return None,
	};
	let epoch = match doc.get_property("epoch") {
		Some(Value::U64(value)) => *value,
		_ => return None,
	};
	let seq = match doc.get_property("seq") {
		Some(Value::U64(value)) => *value,
		_ => return None,
	};
	let len_chars = match doc.get_property("len_chars") {
		Some(Value::U64(value)) => *value,
		_ => return None,
	};
	let language = match doc.get_property("language") {
		Some(Value::String(value)) => value.clone(),
		_ => String::new(),
	};

	Some(DocValues {
		uri,
		epoch,
		seq,
		len_chars,
		language,
	})
}

fn chunk_texts(
	storage: &helix_db::helix_engine::storage_core::HelixGraphStorage,
	uri: &str,
) -> Vec<String> {
	let arena = Bump::new();
	let txn = storage
		.graph_env
		.read_txn()
		.map_err(helix_db::helix_engine::types::EngineError::from)
		.expect("read txn");
	G::new(storage, &txn, &arena)
		.n_from_index("Chunk", "doc_uri", &uri)
		.filter_map(|entry| entry.ok())
		.filter_map(|tv| match tv {
			TraversalValue::Node(node) => node.get_property("text").and_then(|v| match v {
				Value::String(value) => Some(value.clone()),
				_ => None,
			}),
			_ => None,
		})
		.collect()
}

#[test]
fn test_chunk_text_basic() {
	let text = "aa\nbb\ncc\n";
	let chunks = chunk_text(text, 4);
	assert_eq!(chunks.len(), 3);
	assert_eq!(chunks[0].text, "aa\n");
	assert_eq!(chunks[0].start_char, 0);
	assert_eq!(chunks[0].end_char, 3);
	assert_eq!(chunks[1].text, "bb\n");
	assert_eq!(chunks[1].start_char, 3);
	assert_eq!(chunks[1].end_char, 6);
	assert_eq!(chunks[2].text, "cc\n");
	assert_eq!(chunks[2].start_char, 6);
	assert_eq!(chunks[2].end_char, 9);
}

#[test]
fn test_chunk_text_long_line() {
	let text = "abcdefghij\n";
	let chunks = chunk_text(text, 5);
	assert_eq!(chunks.len(), 1);
	assert_eq!(chunks[0].text, text);
	assert_eq!(chunks[0].start_char, 0);
	assert_eq!(chunks[0].end_char, 11);
}

#[test]
fn test_index_document() {
	let (_temp, core) = open_test_core();
	index_document(
		core.storage(),
		"file:///test.rs",
		&Rope::from("hello\nworld\n"),
		1,
		2,
		"rust",
		None,
	)
	.expect("index document");

	let values = read_doc_values(core.storage(), "file:///test.rs").expect("doc values");
	assert_eq!(values.uri, "file:///test.rs");
	assert_eq!(values.epoch, 1);
	assert_eq!(values.seq, 2);
	assert_eq!(values.len_chars, 12);
	assert_eq!(values.language, "rust");

	let chunks = chunk_texts(core.storage(), "file:///test.rs");
	assert_eq!(chunks.len(), 1);
}

#[test]
fn test_chunk_cleanup_on_reindex() {
	let (_temp, core) = open_test_core();
	let uri = "file:///cleanup.rs";

	index_document(
		core.storage(),
		uri,
		&Rope::from("a\nb\nc\n"),
		1,
		1,
		"",
		None,
	)
	.expect("index first");
	let first_count = chunk_texts(core.storage(), uri).len();
	assert!(first_count > 0);

	index_document(core.storage(), uri, &Rope::from("short\n"), 1, 2, "", None).expect("reindex");
	let second_count = chunk_texts(core.storage(), uri).len();
	assert!(second_count <= first_count);
}

#[test]
fn test_index_updates_on_edit() {
	let (_temp, core) = open_test_core();
	let uri = "file:///edit.rs";

	index_document(
		core.storage(),
		uri,
		&Rope::from("old unique content\n"),
		1,
		1,
		"",
		None,
	)
	.expect("index old");
	index_document(
		core.storage(),
		uri,
		&Rope::from("new unique content\n"),
		1,
		2,
		"",
		None,
	)
	.expect("index new");

	let chunks = chunk_texts(core.storage(), uri);
	assert!(
		chunks
			.iter()
			.any(|text| text.contains("new unique content"))
	);
	assert!(
		chunks
			.iter()
			.all(|text| !text.contains("old unique content"))
	);
}

#[test]
fn test_crawler_reindexes_when_mtime_changes_even_if_epoch_seq_same() {
	let (_temp, core) = open_test_core();
	let uri = "file:///mtime_reindex.rs";

	// Initial index with mtime=10
	index_document(
		core.storage(),
		uri,
		&Rope::from("content 1"),
		0,
		0,
		"",
		Some(10),
	)
	.expect("index 1");
	let chunks = chunk_texts(core.storage(), uri);
	assert_eq!(chunks[0], "content 1");

	// Reindex with same epoch/seq but different mtime=11
	index_document(
		core.storage(),
		uri,
		&Rope::from("content 2"),
		0,
		0,
		"",
		Some(11),
	)
	.expect("index 2");
	let chunks = chunk_texts(core.storage(), uri);
	assert_eq!(chunks[0], "content 2");

	// Reindex with same mtime should be ignored
	index_document(
		core.storage(),
		uri,
		&Rope::from("content 3"),
		0,
		0,
		"",
		Some(11),
	)
	.expect("index 3");
	let chunks = chunk_texts(core.storage(), uri);
	assert_eq!(chunks[0], "content 2");
}
#[test]
fn test_stale_index_discarded() {
	let (_temp, core) = open_test_core();
	let uri = "file:///stale.rs";

	index_document(core.storage(), uri, &Rope::from("fresh\n"), 2, 4, "", None)
		.expect("index fresh");
	index_document(core.storage(), uri, &Rope::from("stale\n"), 1, 1, "", None)
		.expect("index stale");

	let values = read_doc_values(core.storage(), uri).expect("doc values");
	assert_eq!(values.epoch, 2);
	assert_eq!(values.seq, 4);

	let combined = chunk_texts(core.storage(), uri).concat();
	assert!(combined.contains("fresh"));
	assert!(!combined.contains("stale"));
}

#[tokio::test]
async fn test_dirty_mark_is_nonblocking() {
	let temp = TempDir::new().expect("tempdir");
	let storage = helix_db::helix_engine::storage_core::HelixGraphStorage::new(
		temp.path().to_str().unwrap(),
		db::schema_config().clone(),
		Default::default(),
	)
	.expect("storage");
	let source: Arc<dyn DocSnapshotSource> = Arc::new(NullSource);
	let worker = IndexWorker::spawn(Arc::new(storage), Arc::downgrade(&source));

	let start = std::time::Instant::now();
	worker.mark_dirty("file:///test.rs".to_string());
	assert!(start.elapsed() < std::time::Duration::from_millis(10));
}

#[test]
fn test_search_empty_index() {
	let (_temp, core) = open_test_core();
	let hits = core.search("anything", 10).expect("search");
	assert!(hits.is_empty());
}

#[test]
fn test_search_finds_indexed_content() {
	let (_temp, core) = open_test_core();
	index_document(
		core.storage(),
		"file:///alpha.rs",
		&Rope::from("alpha beta"),
		1,
		1,
		"",
		None,
	)
	.expect("index alpha");
	index_document(
		core.storage(),
		"file:///unique.rs",
		&Rope::from("unique_term present"),
		1,
		1,
		"",
		None,
	)
	.expect("index unique");

	let hits = core.search("unique_term", 10).expect("search");
	assert!(hits.iter().any(|hit| hit.uri == "file:///unique.rs"));
}

#[test]
fn test_search_ranking() {
	let (_temp, core) = open_test_core();
	index_document(
		core.storage(),
		"file:///dense.rs",
		&Rope::from("apple apple apple banana"),
		1,
		1,
		"",
		None,
	)
	.expect("index dense");
	index_document(
		core.storage(),
		"file:///sparse.rs",
		&Rope::from("apple banana"),
		1,
		1,
		"",
		None,
	)
	.expect("index sparse");

	let hits = core.search("apple", 2).expect("search");
	assert_eq!(
		hits.first().map(|hit| hit.uri.as_str()),
		Some("file:///dense.rs")
	);
}

#[test]
fn test_search_returns_only_chunks() {
	let (_temp, core) = open_test_core();
	index_document(
		core.storage(),
		"file:///lang.rs",
		&Rope::from("hello world"),
		1,
		1,
		"rust",
		None,
	)
	.expect("index doc");

	let hits = core.search("rust", 10).expect("search");
	assert!(hits.is_empty());
}
