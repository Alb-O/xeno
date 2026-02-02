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
use tokio::sync::Notify;

use super::KnowledgeError;

const LABEL_DOC: &str = "Doc";
const LABEL_CHUNK: &str = "Chunk";
const INDEX_DOC_URI: &str = "uri";
const INDEX_CHUNK_URI: &str = "doc_uri";
const CHUNK_TARGET_CHARS: usize = 2000;
const INDEX_DEBOUNCE_MS: u64 = 500;

/// Background worker that coalesces dirty documents and reindexes them.
pub struct IndexWorker {
	dirty: Arc<Mutex<HashSet<String>>>,
	notify: Arc<Notify>,
}

impl IndexWorker {
	pub fn spawn(storage: Arc<HelixGraphStorage>, broker: Weak<super::super::BrokerCore>) -> Self {
		let dirty: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
		let notify = Arc::new(Notify::new());

		let dirty_task = Arc::clone(&dirty);
		let notify_task = Arc::clone(&notify);

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

				let Some(core) = broker.upgrade() else {
					break;
				};

				for uri in uris {
					let Some((epoch, seq, rope)) = core.snapshot_sync_doc(&uri) else {
						continue;
					};

					if let Err(err) =
						index_document(&storage, &uri, &rope.to_string(), epoch.0, seq.0, "")
					{
						tracing::warn!(error = %err, ?uri, "knowledge index failed");
						continue;
					}

					if let Some((new_epoch, new_seq, _)) = core.snapshot_sync_doc(&uri)
						&& (new_epoch != epoch || new_seq != seq)
					{
						let mut set = dirty_task.lock().unwrap();
						set.insert(uri);
						notify_task.notify_one();
					}
				}
			}
		});

		Self { dirty, notify }
	}

	pub fn mark_dirty(&self, uri: String) {
		let mut set = self.dirty.lock().unwrap();
		set.insert(uri);
		self.notify.notify_one();
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
	pub chunk_idx: u32,
	pub start_char: u64,
	pub end_char: u64,
	pub text: String,
}

pub fn chunk_text(text: &str, target_size: usize) -> Vec<TextChunk> {
	if text.is_empty() {
		return Vec::new();
	}

	let mut chunks = Vec::new();
	let mut current = String::new();
	let mut current_start = 0u64;
	let mut current_len = 0usize;
	let mut cursor = 0u64;
	let mut idx = 0u32;

	for line in text.split_inclusive('\n') {
		let line_len = line.chars().count();

		if current_len > 0 && current_len + line_len > target_size {
			let end_char = current_start + current_len as u64;
			chunks.push(TextChunk {
				chunk_idx: idx,
				start_char: current_start,
				end_char,
				text: current.clone(),
			});
			idx += 1;
			current.clear();
			current_len = 0;
		}

		if current_len == 0 {
			current_start = cursor;
		}

		if line_len > target_size && current_len == 0 {
			let end_char = cursor + line_len as u64;
			chunks.push(TextChunk {
				chunk_idx: idx,
				start_char: cursor,
				end_char,
				text: line.to_string(),
			});
			idx += 1;
			cursor = end_char;
			continue;
		}

		current.push_str(line);
		current_len += line_len;
		cursor += line_len as u64;
	}

	if current_len > 0 {
		chunks.push(TextChunk {
			chunk_idx: idx,
			start_char: current_start,
			end_char: current_start + current_len as u64,
			text: current,
		});
	}

	chunks
}

pub fn index_document(
	storage: &HelixGraphStorage,
	uri: &str,
	text: &str,
	epoch: u64,
	seq: u64,
	language: &str,
) -> Result<(), KnowledgeError> {
	let arena = Bump::new();

	let read_txn = storage
		.graph_env
		.read_txn()
		.map_err(helix_db::helix_engine::types::EngineError::from)?;
	let mut existing_doc: Option<TraversalValue<'_>> = None;
	let mut existing_epoch = None;
	let mut existing_seq = None;

	for entry in G::new(storage, &read_txn, &arena).n_from_index(LABEL_DOC, INDEX_DOC_URI, &uri) {
		if let Ok(TraversalValue::Node(node)) = entry {
			if let Some(Value::U64(value)) = node.get_property("epoch") {
				existing_epoch = Some(*value);
			}
			if let Some(Value::U64(value)) = node.get_property("seq") {
				existing_seq = Some(*value);
			}
			existing_doc = Some(TraversalValue::Node(node));
			break;
		}
	}

	if let (Some(current_epoch), Some(current_seq)) = (existing_epoch, existing_seq)
		&& (current_epoch > epoch || (current_epoch == epoch && current_seq >= seq))
	{
		return Ok(());
	}

	let chunk_nodes: Vec<TraversalValue<'_>> = G::new(storage, &read_txn, &arena)
		.n_from_index(LABEL_CHUNK, INDEX_CHUNK_URI, &uri)
		.filter_map(|entry| entry.ok())
		.filter_map(|tv| match tv {
			TraversalValue::Node(node) => Some(TraversalValue::Node(node)),
			_ => None,
		})
		.collect();

	drop(read_txn);

	let mut write_txn = storage
		.graph_env
		.write_txn()
		.map_err(helix_db::helix_engine::types::EngineError::from)?;

	if !chunk_nodes.is_empty() {
		Drop::drop_traversal(chunk_nodes.into_iter().map(Ok), storage, &mut write_txn)?;
	}

	let len_chars = text.chars().count() as u64;
	let props = vec![
		("uri", Value::String(uri.to_string())),
		("epoch", Value::U64(epoch)),
		("seq", Value::U64(seq)),
		("len_chars", Value::U64(len_chars)),
		("language", Value::String(language.to_string())),
	];

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

	for chunk in chunk_text(text, CHUNK_TARGET_CHARS) {
		let props = super::build_props(
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
