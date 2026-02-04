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
	CHECKPOINT_STRIDE, INDEX_DOC_URI, INDEX_SHARED_URI, LABEL_HISTORY_NODE, LABEL_SHARED_DOC,
	checkpoint_compact_linear, find_shared_doc, load_history_nodes, prune_cleared_branches,
	shared_doc_key, try_reconstruct_chain, upsert_history_node, upsert_shared_doc,
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

	/// Returns the shared helix-db storage handle.
	pub fn storage(&self) -> Arc<HelixGraphStorage> {
		self.storage.clone()
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
			head_group_id: doc.head_group_id,
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

		let mut best_chain: Option<(u64, u64, Rope, usize)> = None;

		for leaf in leaves {
			match try_reconstruct_chain(root.node_id, leaf.node_id, &nodes, &root.root_text) {
				Ok((rope, chain)) => {
					let chain_len = chain.len();
					if best_chain
						.as_ref()
						.is_none_or(|(_, _, _, bl)| chain_len > *bl)
					{
						best_chain = Some((leaf.node_id, leaf.group_id, rope, chain_len));
					}
				}
				Err(err) => {
					tracing::debug!(?uri, leaf = leaf.node_id, error = %err, "skipping invalid leaf during repair");
				}
			}
		}

		let Some((head_id, head_group_id, rope, _)) = best_chain else {
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
			head_group_id,
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
			head_group_id: 0,
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
			0,
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
	#[allow(clippy::too_many_arguments)]
	pub fn append_edit_with_checkpoint(
		&self,
		uri: &str,
		meta: &mut HistoryMeta,
		epoch: SyncEpoch,
		seq: SyncSeq,
		hash64: u64,
		len_chars: u64,
		group_id: u64,
		author_sid: u64,
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
			group_id,
			author_sid,
			redo_tx,
			undo_tx,
			hash64,
			len_chars,
			false,
			String::new(),
		)?;

		new_meta.head_id = node_id;
		new_meta.head_group_id = group_id;
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

	/// Loads the combined undo transaction for the current head's group.
	pub fn load_undo_group(
		&self,
		uri: &str,
		meta: &HistoryMeta,
		rope: &Rope,
	) -> Result<Option<(u64, u64, WireTx)>, HistoryError> {
		if meta.head_id == 0 || meta.head_id == meta.root_id {
			return Ok(None);
		}
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn()?;
		let nodes = load_history_nodes(&self.storage, &txn, &arena, uri)?;

		let mut current_id = meta.head_id;
		let Some(head_node) = nodes.get(&current_id) else {
			return Ok(None);
		};
		let gid = head_node.group_id;

		let mut working_rope = rope.clone();
		let mut last_parent = current_id;

		while current_id != 0 {
			let Some(node) = nodes.get(&current_id) else {
				break;
			};
			if node.group_id != gid || node.is_root {
				break;
			}

			let undo_tx = node.undo_tx()?;
			let tx = crate::wire_convert::wire_to_tx(&undo_tx, working_rope.slice(..))
				.map_err(|_| HistoryError::InvalidDelta)?;
			tx.apply(&mut working_rope);

			last_parent = node.parent_id;
			current_id = node.parent_id;
		}

		if last_parent == meta.head_id {
			return Ok(None);
		}

		let delta = crate::wire_convert::rope_delta(rope, &working_rope);
		let wire = crate::wire_convert::tx_to_wire(&delta);

		let prev_group = nodes.get(&last_parent).map(|n| n.group_id).unwrap_or(0);

		Ok(Some((last_parent, prev_group, wire)))
	}

	/// Loads the combined redo transaction for the next child group.
	pub fn load_redo_group(
		&self,
		uri: &str,
		meta: &HistoryMeta,
		rope: &Rope,
	) -> Result<Option<(u64, u64, WireTx)>, HistoryError> {
		let arena = Bump::new();
		let txn = self.storage.graph_env.read_txn()?;
		let nodes = load_history_nodes(&self.storage, &txn, &arena, uri)?;

		// Redo requires finding a child of head_id.
		// Since there might be multiple branches, we pick the one with highest node_id.
		let mut next_id = 0;
		for node in nodes.values() {
			if node.parent_id == meta.head_id && !node.is_root && node.node_id > next_id {
				next_id = node.node_id;
			}
		}

		if next_id == 0 {
			return Ok(None);
		}

		let gid = nodes.get(&next_id).unwrap().group_id;
		let mut current_id = next_id;
		let mut working_rope = rope.clone();
		let mut last_id = current_id;

		while current_id != 0 {
			let Some(node) = nodes.get(&current_id) else {
				break;
			};
			if node.group_id != gid || node.is_root {
				break;
			}

			let redo_tx = node.redo_tx()?;
			let tx = crate::wire_convert::wire_to_tx(&redo_tx, working_rope.slice(..))
				.map_err(|_| HistoryError::InvalidDelta)?;
			tx.apply(&mut working_rope);

			last_id = node.node_id;

			// Find next child in same group
			let mut best_child = 0;
			for candidate in nodes.values() {
				if candidate.parent_id == current_id
					&& candidate.group_id == gid
					&& !candidate.is_root
					&& candidate.node_id > best_child
				{
					best_child = candidate.node_id;
				}
			}
			current_id = best_child;
		}

		let delta = crate::wire_convert::rope_delta(rope, &working_rope);
		let wire = crate::wire_convert::tx_to_wire(&delta);

		Ok(Some((last_id, gid, wire)))
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

	/// Internal helper for tests to simulate metadata loss.
	#[cfg(test)]
	pub fn purge_doc_metadata_only(&self, uri: &str) -> Result<(), HistoryError> {
		let arena = Bump::new();
		let mut txn = self.storage.graph_env.write_txn()?;
		let key = shared_doc_key(uri);
		super::internals::drop_by_index(
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
			0,
			0,
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
			0,
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
