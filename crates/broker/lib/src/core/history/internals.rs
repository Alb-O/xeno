use std::collections::{HashMap, HashSet};

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

use super::types::{HistoryError, HistoryMeta};
use crate::wire_convert;

pub(super) const LABEL_SHARED_DOC: &str = "SharedDoc";
pub(super) const LABEL_HISTORY_NODE: &str = "HistoryNode";
pub(super) const INDEX_SHARED_URI: &str = "shared_uri";
pub(super) const INDEX_NODE_KEY: &str = "node_key";
pub(super) const INDEX_DOC_URI: &str = "history_uri";

/// How many edits to compact into a new root per checkpoint.
pub(super) const CHECKPOINT_STRIDE: usize = 25;

pub(super) struct SharedDocRecord {
	pub(super) epoch: SyncEpoch,
	pub(super) seq: SyncSeq,
	pub(super) len_chars: u64,
	pub(super) hash64: u64,
	pub(super) head_node_id: u64,
	pub(super) root_node_id: u64,
	pub(super) next_node_id: u64,
	pub(super) history_nodes: u64,
	pub(super) head_group_id: u64,
}

#[derive(Clone)]
pub(super) struct HistoryNodeRecord {
	pub(super) node_id: u64,
	pub(super) parent_id: u64,
	pub(super) redo_tx_raw: String,
	pub(super) undo_tx_raw: String,
	pub(super) is_root: bool,
	pub(super) root_text: String,
	pub(super) group_id: u64,
	pub(super) author_sid: u64,
}

impl HistoryNodeRecord {
	pub(super) fn redo_tx(&self) -> Result<WireTx, HistoryError> {
		Ok(serde_json::from_str(&self.redo_tx_raw)?)
	}

	#[allow(dead_code)]
	pub(super) fn undo_tx(&self) -> Result<WireTx, HistoryError> {
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
	let hash = xxhash_rust::xxh3::xxh3_64(uri.as_bytes());
	format!("{:016x}#{:016x}", hash, node_id)
}

pub(super) fn shared_doc_key(uri: &str) -> String {
	format!("shared::{uri}")
}

/// Drop all nodes for (label,index,key). Safe if empty.
pub(super) fn drop_by_index<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	label: &str,
	index: &str,
	key: &str,
) -> Result<(), HistoryError> {
	let key_s = key.to_string();
	let mut to_drop: Vec<Result<TraversalValue<'_>, EngineError>> = Vec::new();

	for entry in G::new(storage, txn, arena).n_from_index(label, index, &key_s) {
		let tv = entry?;
		if let TraversalValue::Node(node) = tv {
			to_drop.push(Ok(TraversalValue::Node(node)));
		}
	}

	if !to_drop.is_empty() {
		Drop::drop_traversal(to_drop.into_iter(), storage, txn)?;
	}
	Ok(())
}

pub(super) fn find_shared_doc(
	storage: &HelixGraphStorage,
	txn: &RoTxn,
	arena: &Bump,
	uri: &str,
) -> Result<Option<SharedDocRecord>, HistoryError> {
	let key_new = shared_doc_key(uri);
	let key_old = uri.to_string();

	for key in [&key_new, &key_old] {
		for entry in
			G::new(storage, txn, arena).n_from_index(LABEL_SHARED_DOC, INDEX_SHARED_URI, key)
		{
			let tv = entry?;
			let TraversalValue::Node(node) = tv else {
				continue;
			};

			let epoch = get_u64_lossless(&node, "epoch")
				.map(SyncEpoch)
				.unwrap_or(SyncEpoch(1));
			let seq = get_u64_lossless(&node, "seq")
				.map(SyncSeq)
				.unwrap_or(SyncSeq(0));
			let len_chars = get_u64_lossless(&node, "len_chars").unwrap_or(0);
			let hash64 = get_u64_lossless(&node, "hash64").unwrap_or(0);
			let head_node_id = get_u64_lossless(&node, "head_node_id").unwrap_or(1);
			let root_node_id = get_u64_lossless(&node, "root_node_id").unwrap_or(1);
			let next_node_id = get_u64_lossless(&node, "next_node_id").unwrap_or(2);
			let history_nodes = get_u64_lossless(&node, "history_nodes").unwrap_or(1);
			let head_group_id = get_u64_lossless(&node, "head_group_id").unwrap_or(0);

			return Ok(Some(SharedDocRecord {
				epoch,
				seq,
				len_chars,
				hash64,
				head_node_id,
				root_node_id,
				next_node_id,
				history_nodes,
				head_group_id,
			}));
		}
	}
	Ok(None)
}

pub(super) fn find_history_node(
	storage: &HelixGraphStorage,
	txn: &RoTxn,
	arena: &Bump,
	uri: &str,
	node_id: u64,
) -> Result<Option<HistoryNodeRecord>, HistoryError> {
	let key = node_key(uri, node_id);
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_NODE_KEY, &key)
	{
		let tv = entry?;
		let TraversalValue::Node(node) = tv else {
			continue;
		};
		return Ok(Some(parse_history_node(&node)));
	}
	Ok(None)
}

pub(super) fn load_history_nodes(
	storage: &HelixGraphStorage,
	txn: &RoTxn,
	arena: &Bump,
	uri: &str,
) -> Result<HashMap<u64, HistoryNodeRecord>, HistoryError> {
	let mut nodes = HashMap::new();
	let key = uri.to_string();
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_DOC_URI, &key) {
		let tv = entry?;
		let TraversalValue::Node(node) = tv else {
			continue;
		};
		let record = parse_history_node(&node);
		nodes.insert(record.node_id, record);
	}
	Ok(nodes)
}

fn parse_history_node(node: &Node<'_>) -> HistoryNodeRecord {
	let node_id = get_u64_lossless(node, "node_id").unwrap_or(0);
	let parent_id = get_u64_lossless(node, "parent_id").unwrap_or(0);
	let redo_tx_raw = get_string(node, "redo_tx").unwrap_or_default();
	let undo_tx_raw = get_string(node, "undo_tx").unwrap_or_default();
	let is_root = get_bool(node, "is_root").unwrap_or(false);
	let root_text = get_string(node, "root_text").unwrap_or_default();
	let group_id = get_u64_lossless(node, "group_id").unwrap_or(0);
	let author_sid = get_u64_lossless(node, "author_sid").unwrap_or(0);
	HistoryNodeRecord {
		node_id,
		parent_id,
		redo_tx_raw,
		undo_tx_raw,
		is_root,
		root_text,
		group_id,
		author_sid,
	}
}

pub(super) fn upsert_shared_doc<'a>(
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
	let key_new = shared_doc_key(uri);
	let key_old = uri;

	drop_by_index(
		storage,
		txn,
		arena,
		LABEL_SHARED_DOC,
		INDEX_SHARED_URI,
		&key_new,
	)?;
	drop_by_index(
		storage,
		txn,
		arena,
		LABEL_SHARED_DOC,
		INDEX_SHARED_URI,
		key_old,
	)?;

	let props = vec![
		("shared_uri", Value::String(key_new)),
		("epoch", Value::U64(epoch.0)),
		("seq", Value::U64(seq.0)),
		("len_chars", Value::U64(len_chars)),
		("hash64", Value::U64(hash64)),
		("head_node_id", Value::U64(meta.head_id)),
		("root_node_id", Value::U64(meta.root_id)),
		("next_node_id", Value::U64(meta.next_id)),
		("history_nodes", Value::U64(meta.history_nodes)),
		("head_group_id", Value::U64(meta.head_group_id)),
	];

	G::new_mut_from_iter(storage, txn, std::iter::empty::<TraversalValue>(), arena)
		.upsert_n(LABEL_SHARED_DOC, &props)
		.collect::<Result<Vec<_>, _>>()?;
	Ok(())
}

pub(super) fn upsert_history_node<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	uri: &str,
	node_id: u64,
	parent_id: u64,
	group_id: u64,
	author_sid: u64,
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
		("history_uri", Value::String(uri.to_string())),
		("node_id", Value::U64(node_id)),
		("parent_id", Value::U64(parent_id)),
		("group_id", Value::U64(group_id)),
		("author_sid", Value::U64(author_sid)),
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

pub(super) fn prune_cleared_branches<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	uri: &str,
	meta: &HistoryMeta,
) -> Result<u64, HistoryError> {
	let mut entries = Vec::new();
	let key = uri.to_string();
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_DOC_URI, &key) {
		if let Ok(TraversalValue::Node(node)) = entry {
			let node_id = get_u64_lossless(&node, "node_id").unwrap_or(0);
			let parent_id = get_u64_lossless(&node, "parent_id").unwrap_or(0);
			entries.push((node_id, parent_id, TraversalValue::Node(node)));
		}
	}

	let mut parent_map = HashMap::new();
	for (node_id, parent_id, _) in &entries {
		parent_map.insert(*node_id, *parent_id);
	}

	let mut ancestry = HashSet::new();
	let mut current = meta.head_id;
	let mut reached_root = false;

	while current != 0 && ancestry.insert(current) {
		if current == meta.root_id {
			reached_root = true;
			break;
		}
		let Some(parent) = parent_map.get(&current) else {
			break;
		};
		current = *parent;
	}

	if !reached_root && meta.head_id != 0 {
		tracing::warn!(?uri, "ancestry walk failed to reach root; skipping prune");
		return Ok(0);
	}

	ancestry.insert(meta.root_id);

	let candidates: Vec<_> = entries
		.into_iter()
		.filter(|(node_id, _, _)| !ancestry.contains(node_id))
		.collect();

	let removed = candidates.len() as u64;
	if !candidates.is_empty() {
		let to_drop = candidates
			.into_iter()
			.map(|(_, _, tv)| Ok::<TraversalValue<'_>, EngineError>(tv));
		Drop::drop_traversal(to_drop, storage, txn)?;
	}

	Ok(removed)
}

pub(super) fn checkpoint_compact_linear<'a>(
	storage: &'a HelixGraphStorage,
	txn: &mut RwTxn<'a>,
	arena: &Bump,
	uri: &str,
	meta: &mut HistoryMeta,
	max_nodes: usize,
	stride: usize,
) -> Result<u64, HistoryError> {
	if max_nodes < 2 {
		return Ok(0);
	}

	let key = uri.to_string();
	let mut nodes: HashMap<u64, (HistoryNodeRecord, TraversalValue)> = HashMap::new();
	for entry in G::new(storage, txn, arena).n_from_index(LABEL_HISTORY_NODE, INDEX_DOC_URI, &key) {
		if let Ok(TraversalValue::Node(node)) = entry {
			let rec = parse_history_node(&node);
			nodes.insert(rec.node_id, (rec, TraversalValue::Node(node)));
		}
	}

	let root_id = meta.root_id;
	let head_id = meta.head_id;

	let mut chain_rev: Vec<u64> = Vec::new();
	let mut cur = head_id;
	let mut seen = HashSet::new();
	while cur != 0 && seen.insert(cur) {
		chain_rev.push(cur);
		if cur == root_id {
			break;
		}
		let Some((rec, _)) = nodes.get(&cur) else {
			break;
		};
		cur = rec.parent_id;
	}

	if chain_rev.last().copied() != Some(root_id) {
		return Ok(0);
	}

	let chain_len = chain_rev.len();
	if chain_len <= max_nodes {
		return Ok(0);
	}

	let overflow = chain_len.saturating_sub(max_nodes);
	let mut compact = overflow.div_ceil(stride) * stride;
	compact = compact.min(chain_len - 1);
	if compact == 0 {
		return Ok(0);
	}

	let new_root_idx_rev = chain_len - 1 - compact;
	let new_root_id = chain_rev[new_root_idx_rev];

	let old_root = nodes
		.get(&root_id)
		.map(|(r, _)| r.clone())
		.ok_or_else(|| HistoryError::Corrupt(format!("missing root node for {uri}")))?;

	if !old_root.is_root {
		return Err(HistoryError::Corrupt(format!(
			"root node missing is_root flag for {uri}"
		)));
	}

	let mut rope = Rope::from(old_root.root_text.as_str());

	for idx in (new_root_idx_rev..chain_len - 1).rev() {
		let node_id = chain_rev[idx];
		if node_id == root_id {
			continue;
		}
		let (rec, _) = nodes
			.get(&node_id)
			.ok_or_else(|| HistoryError::Corrupt(format!("missing node {node_id} for {uri}")))?;
		let redo = rec.redo_tx()?;
		apply_wire_tx(&mut rope, &redo)?;
	}

	let new_root_text = rope.to_string();
	let (len, hash) = xeno_broker_proto::fingerprint_rope(&rope);

	let new_root_rec = nodes.get(&new_root_id).map(|(r, _)| r).unwrap();

	upsert_history_node(
		storage,
		txn,
		arena,
		uri,
		new_root_id,
		0,
		new_root_rec.group_id,
		new_root_rec.author_sid,
		WireTx(Vec::new()),
		WireTx(Vec::new()),
		hash,
		len,
		true,
		new_root_text,
	)?;

	meta.root_id = new_root_id;

	let mut to_drop: Vec<Result<TraversalValue, EngineError>> = Vec::new();
	let mut removed = 0_u64;

	for idx in (new_root_idx_rev + 1)..chain_len {
		let node_id = chain_rev[idx];
		if node_id == new_root_id {
			continue;
		}
		if let Some((_, tv)) = nodes.get(&node_id) {
			to_drop.push(Ok(tv.clone()));
			removed += 1;
		}
	}

	if !to_drop.is_empty() {
		Drop::drop_traversal(to_drop.into_iter(), storage, txn)?;
	}

	Ok(removed)
}

pub(super) fn try_reconstruct_chain(
	root_id: u64,
	head_id: u64,
	nodes: &HashMap<u64, HistoryNodeRecord>,
	root_text: &str,
) -> Result<(Rope, Vec<HistoryNodeRecord>), HistoryError> {
	let mut chain = Vec::new();
	let mut current = head_id;
	let mut seen = HashSet::new();

	while current != root_id {
		if !seen.insert(current) {
			return Err(HistoryError::Corrupt(format!(
				"cycle detected at node {current}"
			)));
		}
		let node = nodes.get(&current).ok_or_else(|| {
			HistoryError::Corrupt(format!("missing node {current} in history chain"))
		})?;
		chain.push(node.clone());
		current = node.parent_id;
	}

	let mut rope = Rope::from(root_text);
	for node in chain.iter().rev() {
		let redo = node.redo_tx()?;
		apply_wire_tx(&mut rope, &redo)?;
	}

	Ok((rope, chain))
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

fn get_u64_lossless(node: &Node<'_>, key: &str) -> Option<u64> {
	match node.get_property(key) {
		Some(Value::U64(value)) => Some(*value),
		Some(Value::U128(value)) => u64::try_from(*value).ok(),
		_ => None,
	}
}
