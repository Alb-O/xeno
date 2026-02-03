//! Broker-owned history store for shared documents.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bumpalo::Bump;
use heed3::{RoTxn, RwTxn};
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::source::n_from_index::NFromIndexAdapter;
use helix_db::helix_engine::traversal_core::ops::util::drop::Drop;
use helix_db::helix_engine::traversal_core::ops::util::upsert::UpsertAdapter;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::helix_engine::types::EngineError;
use helix_db::protocol::value::Value;
use helix_db::utils::items::Node;
use ropey::Rope;
use xeno_broker_proto::types::{SyncEpoch, SyncSeq, WireTx};

use crate::wire_convert;

const LABEL_SHARED_DOC: &str = "SharedDoc";
const LABEL_HISTORY_NODE: &str = "HistoryNode";
const INDEX_SHARED_URI: &str = "uri";
const INDEX_NODE_KEY: &str = "node_key";
const INDEX_DOC_URI: &str = "doc_uri";

/// History store backed by helix-db.
pub struct HistoryStore {
	storage: Arc<HelixGraphStorage>,
}

impl HistoryStore {
	/// Creates a new history store for the given helix-db storage.
	pub fn new(storage: Arc<HelixGraphStorage>) -> Self {
		Self { storage }
	}

	/// Loads an existing document from storage, if present.
	pub fn load_doc(&self, uri: &str) -> Result<Option<StoredDoc>, HistoryError> {
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn()?;

		let Some(doc) = find_shared_doc(&self.storage, &txn, &arena, uri)? else {
			return Ok(None);
		};

		let meta = HistoryMeta {
			head_id: doc.head_node_id,
			root_id: doc.root_node_id,
			next_id: doc.next_node_id,
			history_nodes: doc.history_nodes,
		};

		let nodes = load_history_nodes(&self.storage, &txn, &arena, uri)?;
		let root = nodes
			.get(&doc.root_node_id)
			.ok_or_else(|| HistoryError::Corrupt(format!("missing root node for {uri}")))?;
		if !root.is_root {
			return Err(HistoryError::Corrupt(format!(
				"root node missing is_root flag for {uri}"
			)));
		}

		let mut chain = Vec::new();
		let mut current = doc.head_node_id;
		let mut seen = HashSet::new();

		while current != doc.root_node_id {
			if !seen.insert(current) {
				return Err(HistoryError::Corrupt(format!(
					"cycle in history graph for {uri}"
				)));
			}
			let node = nodes.get(&current).ok_or_else(|| {
				HistoryError::Corrupt(format!("missing node {current} for {uri}"))
			})?;
			chain.push(node.clone());
			current = node.parent_id;
		}

		let mut rope = Rope::from(root.root_text.as_str());
		for node in chain.iter().rev() {
			let redo = node.redo_tx()?;
			apply_wire_tx(&mut rope, &redo)?;
		}

		Ok(Some(StoredDoc {
			meta,
			epoch: doc.epoch,
			seq: doc.seq,
			len_chars: doc.len_chars,
			hash64: doc.hash64,
			rope,
		}))
	}

	/// Creates a new document entry with an initial root node.
	pub fn create_doc(
		&self,
		uri: &str,
		rope: &Rope,
		epoch: SyncEpoch,
		seq: SyncSeq,
		hash64: u64,
		len_chars: u64,
	) -> Result<StoredDoc, HistoryError> {
		let arena = Bump::new();
		let mut txn = self.storage.graph_env.write_txn()?;

		let root_id = 1_u64;
		let meta = HistoryMeta {
			head_id: root_id,
			root_id,
			next_id: root_id + 1,
			history_nodes: 1,
		};

		upsert_shared_doc(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			epoch,
			seq,
			hash64,
			len_chars,
			&meta,
		)?;

		upsert_history_node(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			root_id,
			0,
			WireTx(Vec::new()),
			WireTx(Vec::new()),
			hash64,
			len_chars,
			true,
			rope.to_string(),
		)?;

		txn.commit()?;

		Ok(StoredDoc {
			meta,
			epoch,
			seq,
			len_chars,
			hash64,
			rope: rope.clone(),
		})
	}

	/// Appends a new history node for an edit and updates document metadata.
	pub fn append_edit(
		&self,
		uri: &str,
		meta: &mut HistoryMeta,
		epoch: SyncEpoch,
		seq: SyncSeq,
		hash64: u64,
		len_chars: u64,
		redo_tx: WireTx,
		undo_tx: WireTx,
		max_nodes: usize,
	) -> Result<(), HistoryError> {
		let arena = Bump::new();
		let mut txn = self.storage.graph_env.write_txn()?;

		let node_id = meta.next_id;
		let parent_id = meta.head_id;

		upsert_history_node(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			node_id,
			parent_id,
			redo_tx,
			undo_tx,
			hash64,
			len_chars,
			false,
			String::new(),
		)?;

		meta.head_id = node_id;
		meta.next_id = meta.next_id.saturating_add(1);
		meta.history_nodes = meta.history_nodes.saturating_add(1);

		if meta.history_nodes as usize > max_nodes {
			let removed =
				prune_history_nodes(&self.storage, &mut txn, &arena, uri, meta, max_nodes)?;
			meta.history_nodes = meta.history_nodes.saturating_sub(removed);
		}

		upsert_shared_doc(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			epoch,
			seq,
			hash64,
			len_chars,
			meta,
		)?;

		txn.commit()?;
		Ok(())
	}

	/// Loads the undo transaction for the current head.
	pub fn load_undo(
		&self,
		uri: &str,
		head_id: u64,
	) -> Result<Option<(u64, WireTx)>, HistoryError> {
		if head_id == 0 {
			return Ok(None);
		}
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn()?;
		let node = find_history_node(&self.storage, &txn, &arena, uri, head_id)?;
		let Some(node) = node else {
			return Ok(None);
		};
		if node.is_root {
			return Ok(None);
		}
		Ok(Some((node.parent_id, node.undo_tx()?)))
	}

	/// Loads the redo transaction for the most recent child of the head.
	pub fn load_redo(
		&self,
		uri: &str,
		head_id: u64,
	) -> Result<Option<(u64, WireTx)>, HistoryError> {
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn()?;
		let nodes = load_history_nodes(&self.storage, &txn, &arena, uri)?;
		let mut best: Option<(u64, HistoryNodeRecord)> = None;

		for node in nodes.values() {
			if node.parent_id != head_id || node.is_root {
				continue;
			}
			let candidate = (node.node_id, node.clone());
			if let Some((best_id, _)) = &best {
				if node.node_id > *best_id {
					best = Some(candidate);
				}
			} else {
				best = Some(candidate);
			}
		}

		let Some((node_id, node)) = best else {
			return Ok(None);
		};
		Ok(Some((node_id, node.redo_tx()?)))
	}

	/// Persists updated document metadata after undo/redo.
	pub fn update_doc_state(
		&self,
		uri: &str,
		meta: &HistoryMeta,
		epoch: SyncEpoch,
		seq: SyncSeq,
		hash64: u64,
		len_chars: u64,
	) -> Result<(), HistoryError> {
		let arena = Bump::new();
		let mut txn = self.storage.graph_env.write_txn()?;
		upsert_shared_doc(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			epoch,
			seq,
			hash64,
			len_chars,
			meta,
		)?;
		txn.commit()?;
		Ok(())
	}
}

/// Metadata stored for a document's history graph.
#[derive(Debug, Clone)]
pub struct HistoryMeta {
	/// Current head node id.
	pub head_id: u64,
	/// Root node id for the document.
	pub root_id: u64,
	/// Next available node id.
	pub next_id: u64,
	/// Total number of history nodes tracked.
	pub history_nodes: u64,
}

/// Stored document state and history metadata.
#[derive(Debug, Clone)]
pub struct StoredDoc {
	/// Persisted history metadata.
	pub meta: HistoryMeta,
	/// Current epoch for ownership fencing.
	pub epoch: SyncEpoch,
	/// Current sequence number.
	pub seq: SyncSeq,
	/// Document length in chars.
	pub len_chars: u64,
	/// Hash of the document content.
	pub hash64: u64,
	/// Full document content as a rope.
	pub rope: Rope,
}

/// Errors produced by the history store.
#[derive(Debug)]
pub enum HistoryError {
	/// Helix-db storage errors.
	Heed(heed3::Error),
	/// Helix traversal engine errors.
	Engine(EngineError),
	/// Serialization errors for stored deltas.
	Serde(serde_json::Error),
	/// Delta could not be converted or applied.
	InvalidDelta,
	/// History graph is missing required nodes or contains cycles.
	Corrupt(String),
}

impl std::fmt::Display for HistoryError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Heed(err) => write!(f, "{err}"),
			Self::Engine(err) => write!(f, "{err}"),
			Self::Serde(err) => write!(f, "{err}"),
			Self::InvalidDelta => write!(f, "invalid history delta"),
			Self::Corrupt(msg) => write!(f, "{msg}"),
		}
	}
}

impl std::error::Error for HistoryError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Heed(err) => Some(err),
			Self::Engine(err) => Some(err),
			Self::Serde(err) => Some(err),
			_ => None,
		}
	}
}

impl From<EngineError> for HistoryError {
	fn from(err: EngineError) -> Self {
		Self::Engine(err)
	}
}

impl From<heed3::Error> for HistoryError {
	fn from(err: heed3::Error) -> Self {
		Self::Heed(err)
	}
}

impl From<serde_json::Error> for HistoryError {
	fn from(err: serde_json::Error) -> Self {
		Self::Serde(err)
	}
}

#[derive(Clone)]
struct SharedDocRecord {
	epoch: SyncEpoch,
	seq: SyncSeq,
	len_chars: u64,
	hash64: u64,
	head_node_id: u64,
	root_node_id: u64,
	next_node_id: u64,
	history_nodes: u64,
}

#[derive(Clone)]
struct HistoryNodeRecord {
	node_id: u64,
	parent_id: u64,
	redo_tx_raw: String,
	undo_tx_raw: String,
	is_root: bool,
	root_text: String,
}

impl HistoryNodeRecord {
	fn redo_tx(&self) -> Result<WireTx, HistoryError> {
		Ok(serde_json::from_str(&self.redo_tx_raw)?)
	}

	fn undo_tx(&self) -> Result<WireTx, HistoryError> {
		Ok(serde_json::from_str(&self.undo_tx_raw)?)
	}
}

fn apply_wire_tx(rope: &mut Rope, wire: &WireTx) -> Result<(), HistoryError> {
	let tx =
		wire_convert::wire_to_tx(wire, rope.slice(..)).map_err(|_| HistoryError::InvalidDelta)?;
	tx.apply(rope);
	Ok(())
}

fn node_key(uri: &str, node_id: u64) -> String {
	format!("{uri}#{node_id}")
}

fn find_shared_doc(
	storage: &HelixGraphStorage,
	txn: &RoTxn,
	arena: &Bump,
	uri: &str,
) -> Result<Option<SharedDocRecord>, HistoryError> {
	let key = uri.to_string();
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_SHARED_DOC, INDEX_SHARED_URI, &key)
	{
		if let Ok(TraversalValue::Node(node)) = entry {
			let epoch = get_u64(&node, "epoch")
				.map(SyncEpoch)
				.unwrap_or(SyncEpoch(1));
			let seq = get_u64(&node, "seq").map(SyncSeq).unwrap_or(SyncSeq(0));
			let len_chars = get_u64(&node, "len_chars").unwrap_or(0);
			let hash64 = get_u64(&node, "hash64").unwrap_or(0);
			let head_node_id = get_u64(&node, "head_node_id").unwrap_or(1);
			let root_node_id = get_u64(&node, "root_node_id").unwrap_or(1);
			let next_node_id = get_u64(&node, "next_node_id").unwrap_or(2);
			let history_nodes = get_u64(&node, "history_nodes").unwrap_or(1);
			return Ok(Some(SharedDocRecord {
				epoch,
				seq,
				len_chars,
				hash64,
				head_node_id,
				root_node_id,
				next_node_id,
				history_nodes,
			}));
		}
	}
	Ok(None)
}

fn find_history_node(
	storage: &HelixGraphStorage,
	txn: &RoTxn,
	arena: &Bump,
	uri: &str,
	node_id: u64,
) -> Result<Option<HistoryNodeRecord>, HistoryError> {
	let key = node_key(uri, node_id);
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_NODE_KEY, &key)
	{
		if let Ok(TraversalValue::Node(node)) = entry {
			return Ok(Some(parse_history_node(&node)));
		}
	}
	Ok(None)
}

fn load_history_nodes(
	storage: &HelixGraphStorage,
	txn: &RoTxn,
	arena: &Bump,
	uri: &str,
) -> Result<HashMap<u64, HistoryNodeRecord>, HistoryError> {
	let mut nodes = HashMap::new();
	let key = uri.to_string();
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_DOC_URI, &key) {
		if let Ok(TraversalValue::Node(node)) = entry {
			let record = parse_history_node(&node);
			nodes.insert(record.node_id, record);
		}
	}
	Ok(nodes)
}

fn parse_history_node(node: &Node<'_>) -> HistoryNodeRecord {
	let node_id = get_u64(node, "node_id").unwrap_or(0);
	let parent_id = get_u64(node, "parent_id").unwrap_or(0);
	let redo_tx_raw = get_string(node, "redo_tx").unwrap_or_default();
	let undo_tx_raw = get_string(node, "undo_tx").unwrap_or_default();
	let is_root = get_bool(node, "is_root").unwrap_or(false);
	let root_text = get_string(node, "root_text").unwrap_or_default();
	HistoryNodeRecord {
		node_id,
		parent_id,
		redo_tx_raw,
		undo_tx_raw,
		is_root,
		root_text,
	}
}

fn upsert_shared_doc<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	uri: &str,
	epoch: SyncEpoch,
	seq: SyncSeq,
	hash64: u64,
	len_chars: u64,
	meta: &HistoryMeta,
) -> Result<(), HistoryError> {
	let props = vec![
		("uri", Value::String(uri.to_string())),
		("epoch", Value::U64(epoch.0)),
		("seq", Value::U64(seq.0)),
		("len_chars", Value::U64(len_chars)),
		("hash64", Value::U64(hash64)),
		("head_node_id", Value::U64(meta.head_id)),
		("root_node_id", Value::U64(meta.root_id)),
		("next_node_id", Value::U64(meta.next_id)),
		("history_nodes", Value::U64(meta.history_nodes)),
	];

	G::new_mut_from_iter(storage, txn, std::iter::empty::<TraversalValue>(), arena)
		.upsert_n(LABEL_SHARED_DOC, &props)
		.collect::<Result<Vec<_>, _>>()?;
	Ok(())
}

fn upsert_history_node<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	uri: &str,
	node_id: u64,
	parent_id: u64,
	redo_tx: WireTx,
	undo_tx: WireTx,
	hash64: u64,
	len_chars: u64,
	is_root: bool,
	root_text: String,
) -> Result<(), HistoryError> {
	let redo_tx_raw = serde_json::to_string(&redo_tx)?;
	let undo_tx_raw = serde_json::to_string(&undo_tx)?;

	let props = vec![
		("node_key", Value::String(node_key(uri, node_id))),
		("doc_uri", Value::String(uri.to_string())),
		("node_id", Value::U64(node_id)),
		("parent_id", Value::U64(parent_id)),
		("redo_tx", Value::String(redo_tx_raw)),
		("undo_tx", Value::String(undo_tx_raw)),
		("len_chars", Value::U64(len_chars)),
		("hash64", Value::U64(hash64)),
		("is_root", Value::Boolean(is_root)),
		("root_text", Value::String(root_text)),
	];

	G::new_mut_from_iter(storage, txn, std::iter::empty::<TraversalValue>(), arena)
		.upsert_n(LABEL_HISTORY_NODE, &props)
		.collect::<Result<Vec<_>, _>>()?;
	Ok(())
}

fn prune_history_nodes<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	uri: &str,
	meta: &HistoryMeta,
	max_nodes: usize,
) -> Result<u64, HistoryError> {
	let mut entries = Vec::new();
	let key = uri.to_string();
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_DOC_URI, &key) {
		if let Ok(TraversalValue::Node(node)) = entry {
			let node_id = get_u64(&node, "node_id").unwrap_or(0);
			let parent_id = get_u64(&node, "parent_id").unwrap_or(0);
			entries.push((node_id, parent_id, TraversalValue::Node(node)));
		}
	}

	let mut parent_map = HashMap::new();
	for (node_id, parent_id, _) in &entries {
		parent_map.insert(*node_id, *parent_id);
	}

	let mut ancestry = HashSet::new();
	let mut current = meta.head_id;
	while current != 0 && ancestry.insert(current) {
		let Some(parent) = parent_map.get(&current) else {
			break;
		};
		if *parent == 0 {
			break;
		}
		current = *parent;
	}
	ancestry.insert(meta.root_id);

	let mut candidates: Vec<_> = entries
		.into_iter()
		.filter(|(node_id, _, _)| !ancestry.contains(node_id))
		.collect();
	candidates.sort_by_key(|(node_id, _, _)| *node_id);

	let mut removed = 0_u64;
	let mut remaining_to_remove = (meta.history_nodes as usize).saturating_sub(max_nodes);
	if remaining_to_remove == 0 {
		return Ok(0);
	}

	let mut to_drop = Vec::new();
	for (_, _, traversal) in candidates {
		if remaining_to_remove == 0 {
			break;
		}
		to_drop.push(Ok(traversal));
		remaining_to_remove -= 1;
		removed += 1;
	}

	if !to_drop.is_empty() {
		Drop::drop_traversal(to_drop.into_iter(), storage, txn)?;
	}

	Ok(removed)
}

fn get_u64(node: &Node<'_>, key: &str) -> Option<u64> {
	match node.get_property(key) {
		Some(Value::U64(value)) => Some(*value),
		_ => None,
	}
}

fn get_string(node: &Node<'_>, key: &str) -> Option<String> {
	match node.get_property(key) {
		Some(Value::String(value)) => Some(value.clone()),
		_ => None,
	}
}

fn get_bool(node: &Node<'_>, key: &str) -> Option<bool> {
	match node.get_property(key) {
		Some(Value::Boolean(value)) => Some(*value),
		_ => None,
	}
}
