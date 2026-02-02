//! Background indexing worker and text chunking logic.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use bumpalo::Bump;
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::source::add_n::AddNAdapter;
use helix_db::helix_engine::traversal_core::ops::source::n_from_index::NFromIndexAdapter;
use helix_db::helix_engine::traversal_core::ops::util::drop::Drop;
use helix_db::helix_engine::traversal_core::ops::util::upsert::UpsertAdapter;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::protocol::value::Value;
use ropey::Rope;
use tokio::sync::Notify;

use super::{DocSnapshotSource, KnowledgeError};

const LABEL_DOC: &str = "Doc";
const LABEL_CHUNK: &str = "Chunk";
const INDEX_DOC_URI: &str = "uri";
const INDEX_CHUNK_URI: &str = "doc_uri";
const CHUNK_TARGET_CHARS: usize = 2000;
const INDEX_DEBOUNCE_MS: u64 = 500;

/// Background indexing tasks.
pub enum IndexTask {
	/// URI changed in editor.
	DirtyUri(String),
	/// Full file contents from crawler.
	File {
		/// Canonical URI.
		uri: String,
		/// File content.
		rope: Rope,
		/// Language identifier.
		language: String,
		/// Last modified time.
		mtime: u64,
	},
	/// Point-in-time snapshot of an open document.
	Snapshot {
		/// Canonical URI.
		uri: String,
		/// Document content.
		rope: Rope,
		/// Ownership era.
		epoch: u64,
		/// Edit sequence.
		seq: u64,
		/// Optional modification time.
		mtime: Option<u64>,
	},
}

/// Background worker that coalesces dirty documents and reindexes them.
pub struct IndexWorker {
	hi_pri: tokio::sync::mpsc::Sender<IndexTask>,
	bulk: tokio::sync::mpsc::Sender<IndexTask>,
}

impl IndexWorker {
	/// Spawns the worker tasks.
	pub fn spawn(storage: Arc<HelixGraphStorage>, source: Weak<dyn DocSnapshotSource>) -> Self {
		let (hi_pri_tx, mut hi_pri_rx) = tokio::sync::mpsc::channel::<IndexTask>(128);
		let (bulk_tx, mut bulk_rx) = tokio::sync::mpsc::channel::<IndexTask>(256);

		let dirty: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
		let notify = Arc::new(Notify::new());

		let dirty_task = Arc::clone(&dirty);
		let notify_task = Arc::clone(&notify);
		let tx_inner = hi_pri_tx.clone();

		// Dirty coalescing task
		tokio::spawn(async move {
			loop {
				notify_task.notified().await;
				tokio::time::sleep(Duration::from_millis(INDEX_DEBOUNCE_MS)).await;

				let uris = {
					let mut set = dirty_task.lock().unwrap();
					if set.is_empty() {
						continue;
					}
					set.drain().collect::<Vec<_>>()
				};

				let Some(source) = source.upgrade() else {
					break;
				};

				for uri in uris {
					let Some((epoch, seq, rope)) = source.snapshot_sync_doc(&uri).await else {
						continue;
					};

					if let Err(err) = tx_inner.try_send(IndexTask::Snapshot {
						uri: uri.clone(),
						rope,
						epoch: epoch.0,
						seq: seq.0,
						mtime: None,
					}) {
						match err {
							tokio::sync::mpsc::error::TrySendError::Full(_) => {
								// Queue full, put back in dirty set for next tick.
								let mut set = dirty_task.lock().unwrap();
								set.insert(uri);
								notify_task.notify_one();
							}
							tokio::sync::mpsc::error::TrySendError::Closed(_) => break,
						}
					}
				}
			}
		});

		// Single-writer commit task
		let storage_writer = Arc::clone(&storage);
		tokio::spawn(async move {
			let lang_db = xeno_runtime_language::language_db();

			loop {
				let task = tokio::select! {
					biased;
					Some(t) = hi_pri_rx.recv() => t,
					Some(t) = bulk_rx.recv() => t,
					else => break,
				};

				match task {
					IndexTask::DirtyUri(uri) => {
						let mut set = dirty.lock().unwrap();
						set.insert(uri);
						notify.notify_one();
					}
					IndexTask::File {
						uri,
						rope,
						language,
						mtime,
					} => {
						if let Err(err) = index_document(
							&storage_writer,
							&uri,
							&rope,
							0,
							0,
							&language,
							Some(mtime),
						) {
							tracing::warn!(error = %err, ?uri, "crawler index failed");
						}
					}
					IndexTask::Snapshot {
						uri,
						rope,
						epoch,
						seq,
						mtime,
					} => {
						let language = language_name_for_uri(lang_db, &uri).unwrap_or_default();
						if let Err(err) = index_document(
							&storage_writer,
							&uri,
							&rope,
							epoch,
							seq,
							&language,
							mtime,
						) {
							tracing::warn!(error = %err, ?uri, "knowledge index failed");
						}
					}
				}
			}
		});

		Self {
			hi_pri: hi_pri_tx,
			bulk: bulk_tx,
		}
	}

	/// Signals that a document needs re-indexing.
	pub fn mark_dirty(&self, uri: String) {
		let _ = self.hi_pri.try_send(IndexTask::DirtyUri(uri));
	}

	/// Enqueues a file for bulk indexing.
	pub async fn enqueue_file(&self, uri: String, rope: Rope, language: String, mtime: u64) {
		let _ = self
			.bulk
			.send(IndexTask::File {
				uri,
				rope,
				language,
				mtime,
			})
			.await;
	}
}

fn language_name_for_uri(db: &xeno_runtime_language::LanguageDb, uri: &str) -> Option<String> {
	let u = url::Url::parse(uri).ok()?;
	if u.scheme() != "file" {
		return None;
	}

	let path = u.to_file_path().ok()?;
	let ext = path.extension()?.to_str()?;
	let idx = db.index_for_extension(ext)?;
	db.languages()
		.find_map(|(i, data)| (i == idx).then(|| data.name.clone()))
}

/// A searchable segment of document text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
	/// Relative index within the document.
	pub chunk_idx: u32,
	/// Start char offset.
	pub start_char: u64,
	/// End char offset.
	pub end_char: u64,
	/// Chunk content.
	pub text: String,
}

/// Splits text into searchable chunks of approximately `target_size` characters.
pub fn chunk_text(text: &str, target_size: usize) -> Vec<TextChunk> {
	chunk_rope(&Rope::from(text), target_size)
}

/// Splits a rope into searchable chunks of approximately `target_size` characters.
pub fn chunk_rope(rope: &Rope, target_size: usize) -> Vec<TextChunk> {
	if rope.len_chars() == 0 {
		return Vec::new();
	}

	let mut chunks = Vec::new();
	let mut current_chunk = String::new();
	let mut current_chunk_start = 0usize;
	let mut current_len = 0usize;
	let mut current_cursor = 0usize;
	let mut idx = 0u32;

	for line in rope.lines() {
		let line_str = line.to_string();
		let line_len = line.len_chars();

		if current_len > 0 && current_len + line_len > target_size {
			chunks.push(TextChunk {
				chunk_idx: idx,
				start_char: current_chunk_start as u64,
				end_char: (current_chunk_start + current_len) as u64,
				text: std::mem::take(&mut current_chunk),
			});

			idx += 1;
			current_chunk.clear();
			current_chunk_start += current_len;
			current_len = 0;
		}

		if current_len == 0 {
			current_chunk_start = current_cursor;
		}

		if line_len > target_size && current_len == 0 {
			chunks.push(TextChunk {
				chunk_idx: idx,
				start_char: current_cursor as u64,
				end_char: (current_cursor + line_len) as u64,
				text: line_str,
			});
			idx += 1;
			current_cursor += line_len;
			current_chunk_start = current_cursor;
			continue;
		}

		current_chunk.push_str(&line_str);
		current_len += line_len;
		current_cursor += line_len;
	}

	if current_len > 0 {
		chunks.push(TextChunk {
			chunk_idx: idx,
			start_char: current_chunk_start as u64,
			end_char: (current_chunk_start + current_len) as u64,
			text: current_chunk,
		});
	}

	chunks
}

/// Atomically updates a document and its chunks in the graph database.
///
/// # Errors
///
/// Returns `KnowledgeError` if the database transaction fails.
pub fn index_document(
	storage: &HelixGraphStorage,
	uri: &str,
	rope: &Rope,
	epoch: u64,
	seq: u64,
	language: &str,
	mtime: Option<u64>,
) -> Result<(), KnowledgeError> {
	let arena = Bump::new();

	let mut write_txn = storage
		.graph_env
		.write_txn()
		.map_err(helix_db::helix_engine::types::EngineError::from)?;

	let mut existing_doc: Option<TraversalValue<'_>> = None;
	let mut existing_epoch = None;
	let mut existing_seq = None;
	let mut existing_mtime = None;

	for entry in G::new(storage, &write_txn, &arena).n_from_index(LABEL_DOC, INDEX_DOC_URI, &uri) {
		if let Ok(TraversalValue::Node(node)) = entry {
			if let Some(Value::U64(value)) = node.get_property("epoch") {
				existing_epoch = Some(*value);
			}
			if let Some(Value::U64(value)) = node.get_property("seq") {
				existing_seq = Some(*value);
			}
			if let Some(Value::U64(value)) = node.get_property("mtime") {
				existing_mtime = Some(*value);
			}
			existing_doc = Some(TraversalValue::Node(node));
			break;
		}
	}

	if let (Some(current_epoch), Some(current_seq)) = (existing_epoch, existing_seq)
		&& (current_epoch > epoch || (current_epoch == epoch && current_seq >= seq))
	{
		if epoch == 0 && current_epoch == 0 {
			if let Some(mtime) = mtime
				&& let Some(existing_mtime) = existing_mtime
				&& existing_mtime == mtime
			{
				return Ok(());
			}
		} else {
			return Ok(());
		}
	}

	let chunk_nodes: Vec<TraversalValue<'_>> = G::new(storage, &write_txn, &arena)
		.n_from_index(LABEL_CHUNK, INDEX_CHUNK_URI, &uri)
		.filter_map(|entry| entry.ok())
		.filter_map(|tv| match tv {
			TraversalValue::Node(node) => Some(TraversalValue::Node(node)),
			_ => None,
		})
		.collect();

	if !chunk_nodes.is_empty() {
		Drop::drop_traversal(chunk_nodes.into_iter().map(Ok), storage, &mut write_txn)?;
	}

	let len_chars = rope.len_chars() as u64;
	let mut props = vec![
		("uri", Value::String(uri.to_string())),
		("epoch", Value::U64(epoch)),
		("seq", Value::U64(seq)),
		("len_chars", Value::U64(len_chars)),
		("language", Value::String(language.to_string())),
	];
	let final_mtime = match mtime {
		Some(value) => value,
		None => existing_mtime.unwrap_or(0),
	};
	props.push(("mtime", Value::U64(final_mtime)));

	if let Some(existing) = existing_doc {
		G::new_mut_from_iter(storage, &mut write_txn, std::iter::once(existing), &arena)
			.upsert_n(LABEL_DOC, &props)
			.collect::<Result<Vec<_>, _>>()?;
	} else {
		G::new_mut_from_iter(
			storage,
			&mut write_txn,
			std::iter::empty::<TraversalValue>(),
			&arena,
		)
		.upsert_n(LABEL_DOC, &props)
		.collect::<Result<Vec<_>, _>>()?;
	}

	for chunk in chunk_rope(rope, CHUNK_TARGET_CHARS) {
		let props = crate::core::knowledge::build_props(
			&arena,
			vec![
				("doc_uri", Value::String(uri.to_string())),
				("chunk_idx", Value::U32(chunk.chunk_idx)),
				("start_char", Value::U64(chunk.start_char)),
				("end_char", Value::U64(chunk.end_char)),
				("text", Value::String(chunk.text)),
			],
		);
		G::new_mut(storage, &arena, &mut write_txn)
			.add_n(LABEL_CHUNK, Some(props), Some(&[INDEX_CHUNK_URI]))
			.collect::<Result<Vec<_>, _>>()?;
	}

	write_txn
		.commit()
		.map_err(helix_db::helix_engine::types::EngineError::from)?;
	Ok(())
}
