//! Broker-owned history store for shared documents.
//!
//! Implements a branching history graph backed by `helix-db`. This store manages
//! the persistence of document states, including snapshots and deltas, enabling
//! authoritative undo/redo coordination across multiple editor sessions.
//!
//! # Mental Model
//!
//! The history is modeled as a set of linear chains rooted at checkpoints.
//! Periodic compaction squashes older deltas into new root snapshots to bound
//! traversal costs and storage growth.
//!
//! # Invariants
//!
//! - Linear Ancestry: Every non-root node MUST have exactly one parent.
//! - Single Root: A document history graph MUST have exactly one node marked as root.
//! - Authoritative Fingerprint: Every node stores the `hash64` and `len_chars`
//!   representing the state after applying its transaction.

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
const INDEX_SHARED_URI: &str = "shared_uri";
const INDEX_NODE_KEY: &str = "node_key";
const INDEX_DOC_URI: &str = "history_uri";

/// How many edits to compact into a new root per checkpoint.
const CHECKPOINT_STRIDE: usize = 25;

/// Persistent history store utilizing a graph database for document state tracking.
pub struct HistoryStore {
	storage: Arc<HelixGraphStorage>,
}

impl HistoryStore {
	/// Creates a new history store for the given helix-db storage.
	pub fn new(storage: Arc<HelixGraphStorage>) -> Self {
		Self { storage }
	}

	/// Loads an existing document from storage, if present.
	///
	/// Reconstructs the authoritative [`Rope`] by traversing the history graph
	/// from the root checkpoint to the current head node.
	///
	/// # Errors
	///
	/// Returns [`HistoryError::Corrupt`] if the history graph contains cycles,
	/// missing nodes, or multiple roots.
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

		let (rope, _) =
			try_reconstruct_chain(doc.root_node_id, doc.head_node_id, &nodes, &root.root_text)?;

		Ok(Some(StoredDoc {
			meta,
			epoch: doc.epoch,
			seq: doc.seq,
			len_chars: doc.len_chars,
			hash64: doc.hash64,
			rope,
		}))
	}

	/// Loads a document if it exists; otherwise creates it.
	///
	/// Returns `(doc, created)` where `created=true` means a fresh history graph was created.
	/// Automatically attempts to repair orphaned history nodes if the metadata record is
	/// missing but deltas exist.
	///
	/// # Errors
	///
	/// Returns [`HistoryError`] if storage operations fail or if the graph is
	/// unrecoverably corrupt.
	pub fn load_or_create_doc(
		&self,
		uri: &str,
		rope: &Rope,
		epoch: SyncEpoch,
		seq: SyncSeq,
		hash64: u64,
		len_chars: u64,
	) -> Result<(StoredDoc, bool), HistoryError> {
		if let Some(existing) = self.load_doc(uri)? {
			return Ok((existing, false));
		}

		if let Some(repaired) = self.repair_orphaned_doc(uri, epoch, seq)? {
			return Ok((repaired, false));
		}

		match self.create_doc(uri, rope, epoch, seq, hash64, len_chars) {
			Ok(created) => Ok((created, true)),
			Err(err) if err.is_duplicate_key() => {
				if let Some(existing) = self.load_doc(uri)? {
					Ok((existing, false))
				} else {
					self.repair_orphaned_doc(uri, epoch, seq)?
						.map(|r| (r, false))
						.ok_or(err)
				}
			}
			Err(err) => Err(err),
		}
	}

	/// Attempts to reconstruct document metadata from existing history nodes.
	///
	/// This method analyzes orphaned [`LABEL_HISTORY_NODE`]s to identify the root checkpoint
	/// and the most recent branch head. It implements a "best valid leaf" strategy,
	/// attempting to reconstruct the linear history chain from every potential leaf node
	/// and selecting the deepest valid chain.
	///
	/// If the document graph is found to be unrecoverably corrupt (e.g. cycle detected),
	/// this method purges the broken nodes to permit a clean re-initialization.
	///
	/// # Errors
	///
	/// Returns [`HistoryError`] if storage operations fail. Reconstruction failures
	/// are handled by purging and returning `Ok(None)`.
	fn repair_orphaned_doc(
		&self,
		uri: &str,
		epoch: SyncEpoch,
		seq: SyncSeq,
	) -> Result<Option<StoredDoc>, HistoryError> {
		match self.try_repair_orphaned_doc(uri, epoch, seq) {
			Ok(res) => Ok(res),
			Err(err) => {
				tracing::error!(?uri, error = %err, "unrecoverable history corruption; purging doc history");
				self.purge_doc_history(uri)?;
				Ok(None)
			}
		}
	}

	fn try_repair_orphaned_doc(
		&self,
		uri: &str,
		epoch: SyncEpoch,
		seq: SyncSeq,
	) -> Result<Option<StoredDoc>, HistoryError> {
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn()?;
		let nodes = load_history_nodes(&self.storage, &txn, &arena, uri)?;
		if nodes.is_empty() {
			return Ok(None);
		}

		tracing::info!(?uri, count = nodes.len(), "attempting history graph repair");

		let roots: Vec<_> = nodes.values().filter(|n| n.is_root).collect();
		if roots.len() != 1 {
			return Err(HistoryError::Corrupt(format!(
				"invalid root count during repair for {uri}: {}",
				roots.len()
			)));
		}
		let root = roots[0];

		let mut children_map: HashMap<u64, Vec<u64>> = HashMap::new();
		for node in nodes.values() {
			if !node.is_root {
				children_map
					.entry(node.parent_id)
					.or_default()
					.push(node.node_id);
			}
		}

		let mut leaves: Vec<_> = nodes
			.values()
			.filter(|n| !children_map.contains_key(&n.node_id))
			.collect();

		leaves.sort_by_key(|n| std::cmp::Reverse(n.node_id));

		let mut best_chain: Option<(u64, Rope, usize)> = None;

		for leaf in leaves {
			match try_reconstruct_chain(root.node_id, leaf.node_id, &nodes, &root.root_text) {
				Ok((rope, chain)) => {
					let chain_len = chain.len();
					if best_chain.as_ref().is_none_or(|(_, _, bl)| chain_len > *bl) {
						best_chain = Some((leaf.node_id, rope, chain_len));
					}
				}
				Err(err) => {
					tracing::debug!(?uri, leaf = leaf.node_id, error = %err, "skipping invalid leaf during repair");
				}
			}
		}

		let Some((head_id, rope, _)) = best_chain else {
			return Err(HistoryError::Corrupt(format!(
				"no valid history chain found during repair for {uri}"
			)));
		};

		let max_node_id = nodes.keys().copied().max().unwrap_or(head_id);
		let (len_chars, hash64) = xeno_broker_proto::fingerprint_rope(&rope);
		let meta = HistoryMeta {
			head_id,
			root_id: root.node_id,
			next_id: max_node_id.wrapping_add(1).max(1),
			history_nodes: nodes.len() as u64,
		};

		drop(txn);
		let mut wtxn = self.storage.graph_env.write_txn()?;
		upsert_shared_doc(
			&self.storage,
			&mut wtxn,
			&arena,
			uri,
			epoch,
			seq,
			hash64,
			len_chars,
			&meta,
		)?;
		wtxn.commit()?;

		tracing::info!(?uri, head = head_id, "history graph repair successful");

		Ok(Some(StoredDoc {
			meta,
			epoch,
			seq,
			len_chars,
			hash64,
			rope,
		}))
	}

	/// Deletes all history nodes and document metadata for a URI.
	///
	/// # Errors
	///
	/// Returns [`HistoryError`] if storage traversal or deletion fails.
	pub fn purge_doc_history(&self, uri: &str) -> Result<(), HistoryError> {
		let arena = Bump::new();
		let mut txn = self.storage.graph_env.write_txn()?;
		let key_new = shared_doc_key(uri);
		let key_old = uri.to_string();

		let mut to_drop = Vec::new();
		for key in [&key_new, &key_old] {
			for entry in G::new(&self.storage, &txn, &arena).n_from_index(
				LABEL_SHARED_DOC,
				INDEX_SHARED_URI,
				key,
			) {
				if let Ok(TraversalValue::Node(node)) = entry {
					to_drop.push(Ok(TraversalValue::Node(node)));
				}
			}
		}

		for entry in G::new(&self.storage, &txn, &arena).n_from_index(
			LABEL_HISTORY_NODE,
			INDEX_DOC_URI,
			&uri.to_string(),
		) {
			if let Ok(TraversalValue::Node(node)) = entry {
				to_drop.push(Ok(TraversalValue::Node(node)));
			}
		}

		if !to_drop.is_empty() {
			Drop::drop_traversal(to_drop.into_iter(), &self.storage, &mut txn)?;
		}

		txn.commit()?;
		Ok(())
	}

	/// Creates a new document entry with an initial root checkpoint.
	///
	/// # Errors
	///
	/// Returns [`HistoryError`] if storage already contains a record for this URI
	/// or if serialization fails.
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

	/// Appends a new history node and triggers periodic checkpoint compaction.
	///
	/// Performs three operations in a single transaction:
	/// 1. Inserts the new delta node.
	/// 2. Prunes inactive redo branches.
	/// 3. Materializes a new root checkpoint if linear ancestry exceeds `max_nodes`.
	///
	/// # Errors
	///
	/// Returns [`HistoryError`] if persistence or traversal fails. The in-memory
	/// metadata is only updated if the transaction successfully commits.
	pub fn append_edit_with_checkpoint(
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

		let mut new_meta = meta.clone();
		let node_id = new_meta.next_id;
		let parent_id = new_meta.head_id;

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

		new_meta.head_id = node_id;
		new_meta.next_id = new_meta.next_id.wrapping_add(1).max(1);
		new_meta.history_nodes = new_meta.history_nodes.saturating_add(1);

		let removed_branches =
			prune_cleared_branches(&self.storage, &mut txn, &arena, uri, &new_meta)?;
		new_meta.history_nodes = new_meta.history_nodes.saturating_sub(removed_branches);

		let removed_linear = checkpoint_compact_linear(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			&mut new_meta,
			max_nodes,
			CHECKPOINT_STRIDE,
		)?;
		new_meta.history_nodes = new_meta.history_nodes.saturating_sub(removed_linear);

		upsert_shared_doc(
			&self.storage,
			&mut txn,
			&arena,
			uri,
			epoch,
			seq,
			hash64,
			len_chars,
			&new_meta,
		)?;

		txn.commit()?;
		*meta = new_meta;
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
			if best.as_ref().is_none_or(|(bid, _)| node.node_id > *bid) {
				best = Some(candidate);
			}
		}

		let Some((node_id, node)) = best else {
			return Ok(None);
		};
		Ok(Some((node_id, node.redo_tx()?)))
	}

	/// Persists updated document metadata after an undo/redo transition.
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

impl HistoryError {
	/// Returns true if the underlying storage error is a duplicate-key insert (LMDB MDB_KEYEXIST).
	pub fn is_duplicate_key(&self) -> bool {
		let s = self.to_string();
		s.contains("MDB_KEYEXIST") || s.contains("KEYEXIST") || s.contains("Duplicate key")
	}
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

	#[allow(dead_code)]
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
	let hash = xxhash_rust::xxh3::xxh3_64(uri.as_bytes());
	format!("{:016x}#{:016x}", hash, node_id)
}

fn shared_doc_key(uri: &str) -> String {
	format!("shared::{uri}")
}

/// Drop all nodes for (label,index,key). Safe if empty.
fn drop_by_index<'a>(
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

fn find_shared_doc(
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
		let tv = entry?;
		let TraversalValue::Node(node) = tv else {
			continue;
		};
		return Ok(Some(parse_history_node(&node)));
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
		("history_uri", Value::String(uri.to_string())),
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

fn prune_cleared_branches<'a>(
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

fn checkpoint_compact_linear<'a>(
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

	upsert_history_node(
		storage,
		txn,
		arena,
		uri,
		new_root_id,
		0,
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

fn try_reconstruct_chain(
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

impl HistoryStore {
	/// Internal helper for tests to simulate metadata loss.
	#[cfg(test)]
	fn purge_doc_metadata_only(&self, uri: &str) -> Result<(), HistoryError> {
		let arena = Bump::new();
		let mut txn = self.storage.graph_env.write_txn()?;
		let key = shared_doc_key(uri);
		drop_by_index(
			&self.storage,
			&mut txn,
			&arena,
			LABEL_SHARED_DOC,
			INDEX_SHARED_URI,
			&key,
		)?;
		txn.commit()?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use helix_db::helix_engine::storage_core::HelixGraphStorage;
	use tempfile::tempdir;

	use super::*;

	fn setup_storage() -> (Arc<HelixGraphStorage>, tempfile::TempDir) {
		let dir = tempdir().unwrap();
		let mut config = helix_db::helix_engine::traversal_core::config::Config::default();
		config.graph_config = Some(
			helix_db::helix_engine::traversal_core::config::GraphConfig {
				secondary_indices: Some(vec![
					helix_db::helix_engine::types::SecondaryIndex::Unique("shared_uri".to_string()),
					helix_db::helix_engine::types::SecondaryIndex::Unique("node_key".to_string()),
					helix_db::helix_engine::types::SecondaryIndex::Index("history_uri".to_string()),
				]),
			},
		);

		let storage = Arc::new(
			HelixGraphStorage::new(
				dir.path().to_str().unwrap(),
				config,
				helix_db::helix_engine::storage_core::version_info::VersionInfo::default(),
			)
			.unwrap(),
		);
		(storage, dir)
	}

	#[test]
	fn test_repair_orphaned_doc_success() {
		let (storage, _dir) = setup_storage();
		let history = HistoryStore::new(storage);
		let uri = "file:///test.rs";
		let rope = Rope::from_str("initial text");
		let epoch = SyncEpoch(1);
		let seq = SyncSeq(0);
		let (len, hash) = xeno_broker_proto::fingerprint_rope(&rope);

		history
			.create_doc(uri, &rope, epoch, seq, hash, len)
			.unwrap();
		history.purge_doc_metadata_only(uri).unwrap();

		let (repaired, created) = history
			.load_or_create_doc(uri, &rope, epoch, seq, hash, len)
			.unwrap();
		assert!(!created);
		assert_eq!(repaired.rope.to_string(), "initial text");
		assert_eq!(repaired.meta.head_id, 1);
		assert_eq!(repaired.meta.root_id, 1);
	}

	#[test]
	fn test_repair_orphaned_doc_purges_on_corruption() {
		let (storage, _dir) = setup_storage();
		let history = HistoryStore::new(storage.clone());
		let uri = "file:///test.rs";

		let arena = Bump::new();
		let mut txn = storage.graph_env.write_txn().unwrap();
		upsert_history_node(
			&storage,
			&mut txn,
			&arena,
			uri,
			2,
			99, // missing parent
			WireTx(Vec::new()),
			WireTx(Vec::new()),
			0,
			0,
			false,
			String::new(),
		)
		.unwrap();
		upsert_history_node(
			&storage,
			&mut txn,
			&arena,
			uri,
			1,
			0,
			WireTx(Vec::new()),
			WireTx(Vec::new()),
			0,
			0,
			true,
			"root text".to_string(),
		)
		.unwrap();
		txn.commit().unwrap();

		let rope = Rope::from_str("irrelevant");
		let (stored, created) = history
			.load_or_create_doc(uri, &rope, SyncEpoch(1), SyncSeq(0), 0, 0)
			.unwrap();

		assert_eq!(stored.rope.to_string(), "root text");
		assert!(!created);
	}
}
