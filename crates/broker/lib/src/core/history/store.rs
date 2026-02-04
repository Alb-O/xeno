use std::collections::HashMap;
use std::sync::Arc;

use bumpalo::Bump;
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::source::n_from_index::NFromIndexAdapter;
use helix_db::helix_engine::traversal_core::ops::util::drop::Drop;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use ropey::Rope;
use xeno_broker_proto::types::{SyncEpoch, SyncSeq, WireTx};

use super::internals::{
	checkpoint_compact_linear, find_history_node, find_shared_doc, load_history_nodes,
	prune_cleared_branches, shared_doc_key, try_reconstruct_chain, upsert_history_node,
	upsert_shared_doc, HistoryNodeRecord, CHECKPOINT_STRIDE, INDEX_DOC_URI, INDEX_SHARED_URI,
	LABEL_HISTORY_NODE, LABEL_SHARED_DOC,
};
use super::types::{HistoryError, HistoryMeta, StoredDoc};

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
