//! # Storage Core
//!
//! ## Purpose
//! Persistent graph storage layer backed by LMDB (via heed). Handles raw node/edge CRUD, secondary indexing, and schema migrations.
//!
//! ## Mental model
//! The storage core treats the graph as a set of key-value stores. Nodes and edges are serialized as bytes.
//! Adjacency is maintained via dedicated index databases (out_edges, in_edges).
//!
//! ## Key types
//! | Type | Description |
//! | --- | --- |
//! | `HelixGraphStorage` | Main entry point for storage operations. |
//! | `NodeId` | 128-bit unique identifier for nodes. |
//! | `EdgeId` | 128-bit unique identifier for edges. |
//!
//! ## Invariants
//! - Key/value packing formats are stable and length-checked.
//!   - Enforced in: `HelixGraphStorage::out_edge_key`, `HelixGraphStorage::in_edge_key`, `HelixGraphStorage::pack_edge_data`, `HelixGraphStorage::unpack_adj_edge_data`.
//!   - Tested by: `storage_core::tests::test_pack_unpack_edge_data_roundtrip`, `storage_core::tests::test_out_edge_key_layout`, `storage_core::tests::test_in_edge_key_layout`.
//!   - Failure symptom: Traversal returns wrong adjacency; edges appear missing or swapped.
//! - Secondary index semantics (Unique vs Index) are upheld.
//!   - Enforced in: `ActiveSecondaryIndex::insert`, `ActiveSecondaryIndex::delete`.
//!   - Tested by: `storage_core::tests::test_unique_index_rejects_duplicate`.
//!   - Failure symptom: Duplicate nodes for "unique" lookups; query correctness violation.
//! - LMDB transaction discipline is respected.
//!   - Enforced in: `StorageMethods` implementation.
//!   - Tested by: `storage_concurrent_tests::*`.
//!   - Failure symptom: Deadlocks, `MDB_BAD_TXN`, or stalled writers.
//!
//! ## Data flow
//! 1. Request arrives via `StorageMethods`.
//! 2. Keys are generated using static helpers (e.g., `node_key`).
//! 3. Data is read/written to LMDB via `heed`.
//! 4. Secondary indices are updated atomically within the same transaction.
//!
//! ## Lifecycle
//! - `HelixGraphStorage::new` initializes the environment and ensures all databases exist.
//! - `storage_migration::migrate` runs immediately after opening to align data format with the current version.
//!
//! ## Concurrency & ordering
//! - LMDB provides ACID transactions.
//! - Only one concurrent writer is allowed; multiple concurrent readers are supported.
//!
//! ## Failure modes & recovery
//! - Corrupt database: Recovery requires restoring from backup or re-indexing from source.
//! - Migration failure: The system will refuse to open if metadata indicates an unsupported or partially applied migration.
//!
//! ## Recipes
//! - Adding a secondary index: Define it in `SecondaryIndex` and handle it in `HelixGraphStorage::new`.
//!

pub mod graph_visualization;
pub mod metadata;
pub mod storage_methods;
pub mod storage_migration;
pub mod version_info;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use heed3::byteorder::BE;
use heed3::types::*;
use heed3::{Database, DatabaseFlags, Env, EnvOpenOptions, RoTxn, RwTxn};

use crate::helix_engine::bm25::HBM25Config;
use crate::helix_engine::storage_core::storage_methods::{DBMethods, StorageMethods};
use crate::helix_engine::storage_core::version_info::VersionInfo;
use crate::helix_engine::traversal_core::config::Config;
use crate::helix_engine::types::{
	ActiveSecondaryIndex, EngineError, SecondaryIndex, StorageError, TraversalError,
};
use crate::helix_engine::vector_core::hnsw::HNSW;
use crate::helix_engine::vector_core::vector_core::{HNSWConfig, VectorCore};
use crate::utils::items::{Edge, Node};
use crate::utils::label_hash::hash_label;

// database names for different stores
const DB_NODES: &str = "nodes"; // for node data (n:)
const DB_EDGES: &str = "edges"; // for edge data (e:)
const DB_OUT_EDGES: &str = "out_edges"; // for outgoing edge indices (o:)
const DB_IN_EDGES: &str = "in_edges"; // for incoming edge indices (i:)
const DB_STORAGE_METADATA: &str = "storage_metadata"; // for storage metadata key/value pairs

pub type NodeId = u128;
pub type EdgeId = u128;

pub struct StorageConfig {
	pub schema: Option<String>,
	pub graphvis_node_label: Option<String>,
	pub embedding_model: Option<String>,
}

pub struct HelixGraphStorage {
	pub graph_env: Env,

	pub nodes_db: Database<U128<BE>, Bytes>,
	pub edges_db: Database<U128<BE>, Bytes>,
	pub out_edges_db: Database<Bytes, Bytes>,
	pub in_edges_db: Database<Bytes, Bytes>,
	pub secondary_indices: HashMap<String, (Database<Bytes, U128<BE>>, ActiveSecondaryIndex)>,
	pub vectors: VectorCore,
	pub bm25: Option<HBM25Config>,
	pub metadata_db: Database<Bytes, Bytes>,
	pub version_info: VersionInfo,

	pub storage_config: StorageConfig,
}

impl HelixGraphStorage {
	pub fn new(
		path: &str,
		config: Config,
		version_info: VersionInfo,
	) -> Result<HelixGraphStorage, EngineError> {
		fs::create_dir_all(path)?;

		let db_size = if config.db_max_size_gb.unwrap_or(100) >= 9999 {
			9998
		} else {
			config.db_max_size_gb.unwrap_or(100)
		};

		let graph_env = unsafe {
			EnvOpenOptions::new()
				.map_size(db_size * 1024 * 1024 * 1024)
				.max_dbs(200)
				.max_readers(200)
				.open(Path::new(path))?
		};

		let mut wtxn = graph_env.write_txn()?;

		// creates the lmdb databases (tables)
		// Table: [key]->[value]
		//        [size]->[size]

		// Nodes: [node_id]->[bytes array of node data]
		//        [16 bytes]->[dynamic]
		let nodes_db = graph_env
			.database_options()
			.types::<U128<BE>, Bytes>()
			.name(DB_NODES)
			.create(&mut wtxn)?;

		// Edges: [edge_id]->[bytes array of edge data]
		//        [16 bytes]->[dynamic]
		let edges_db = graph_env
			.database_options()
			.types::<U128<BE>, Bytes>()
			.name(DB_EDGES)
			.create(&mut wtxn)?;

		// Out edges: [from_node_id + label]->[edge_id + to_node_id]  (edge first because value is ordered by byte size)
		//                    [20 + 4 bytes]->[16 + 16 bytes]
		//
		// DUP_SORT used to store all values of duplicated keys under a single key. Saves on space and requires a single read to get all values.
		// DUP_FIXED used to ensure all values are the same size meaning 8 byte length header is discarded.
		let out_edges_db: Database<Bytes, Bytes> = graph_env
			.database_options()
			.types::<Bytes, Bytes>()
			.flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
			.name(DB_OUT_EDGES)
			.create(&mut wtxn)?;

		// In edges: [to_node_id + label]->[edge_id + from_node_id]  (edge first because value is ordered by byte size)
		//                 [20 + 4 bytes]->[16 + 16 bytes]
		//
		// DUP_SORT used to store all values of duplicated keys under a single key. Saves on space and requires a single read to get all values.
		// DUP_FIXED used to ensure all values are the same size meaning 8 byte length header is discarded.
		let in_edges_db: Database<Bytes, Bytes> = graph_env
			.database_options()
			.types::<Bytes, Bytes>()
			.flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
			.name(DB_IN_EDGES)
			.create(&mut wtxn)?;

		let metadata_db: Database<Bytes, Bytes> = graph_env
			.database_options()
			.types::<Bytes, Bytes>()
			.name(DB_STORAGE_METADATA)
			.create(&mut wtxn)?;

		let mut secondary_indices = HashMap::new();
		if let Some(indexes) = config.get_graph_config().secondary_indices {
			for index in indexes {
				let active = match index.into_active() {
					Some(a) => a,
					None => continue,
				};
				match &active {
					ActiveSecondaryIndex::Unique(name) => secondary_indices.insert(
						name.clone(),
						(
							graph_env
								.database_options()
								.types::<Bytes, U128<BE>>()
								.name(name)
								.create(&mut wtxn)?,
							active,
						),
					),
					ActiveSecondaryIndex::Index(name) => secondary_indices.insert(
						name.clone(),
						(
							graph_env
								.database_options()
								.types::<Bytes, U128<BE>>()
								// DUP_SORT used to store all duplicated node keys under a single key.
								//  Saves on space and requires a single read to get all values.
								.flags(DatabaseFlags::DUP_SORT)
								.name(name)
								.create(&mut wtxn)?,
							active,
						),
					),
				};
			}
		}
		let vector_config = config.get_vector_config();
		let vectors = VectorCore::new(
			&graph_env,
			&mut wtxn,
			HNSWConfig::new(
				vector_config.m,
				vector_config.ef_construction,
				vector_config.ef_search,
			),
		)?;

		let bm25 = config
			.get_bm25()
			.then(|| HBM25Config::new(&graph_env, &mut wtxn))
			.transpose()?;

		let storage_config = StorageConfig::new(
			config.schema,
			config.graphvis_node_label,
			config.embedding_model,
		);

		wtxn.commit()?;

		let mut storage = Self {
			graph_env,
			nodes_db,
			edges_db,
			out_edges_db,
			in_edges_db,
			secondary_indices,
			vectors,
			bm25,
			metadata_db,
			storage_config,
			version_info,
		};

		storage_migration::migrate(&mut storage)?;

		Ok(storage)
	}

	/// Used because in the case the key changes in the future.
	/// Believed to not introduce any overhead being inline and using a reference.
	#[must_use]
	#[inline(always)]
	pub fn node_key(id: &u128) -> &u128 {
		id
	}

	/// Used because in the case the key changes in the future.
	/// Believed to not introduce any overhead being inline and using a reference.
	#[must_use]
	#[inline(always)]
	pub fn edge_key(id: &u128) -> &u128 {
		id
	}

	/// Out edge key generator. Creates a 20 byte array and copies in the node id and 4 byte label.
	///
	/// key = `from-node(16)` | `label-id(4)`                 ← 20 B
	///
	/// The generated out edge key will remain the same for the same from_node_id and label.
	/// To save space, the key is only stored once,
	/// with the values being stored in a sorted sub-tree, with this key being the root.
	#[inline(always)]
	pub fn out_edge_key(from_node_id: &u128, label: &[u8; 4]) -> [u8; 20] {
		let mut key = [0u8; 20];
		key[0..16].copy_from_slice(&from_node_id.to_be_bytes());
		key[16..20].copy_from_slice(label);
		key
	}

	/// In edge key generator. Creates a 20 byte array and copies in the node id and 4 byte label.
	///
	/// key = `to-node(16)` | `label-id(4)`                 ← 20 B
	///
	/// The generated in edge key will remain the same for the same to_node_id and label.
	/// To save space, the key is only stored once,
	/// with the values being stored in a sorted sub-tree, with this key being the root.
	#[inline(always)]
	pub fn in_edge_key(to_node_id: &u128, label: &[u8; 4]) -> [u8; 20] {
		let mut key = [0u8; 20];
		key[0..16].copy_from_slice(&to_node_id.to_be_bytes());
		key[16..20].copy_from_slice(label);
		key
	}

	/// Packs the edge data into a 32 byte array.
	///
	/// data = `edge-id(16)` | `node-id(16)`                 ← 32 B (DUPFIXED)
	#[inline(always)]
	pub fn pack_edge_data(edge_id: &u128, node_id: &u128) -> [u8; 32] {
		let mut key = [0u8; 32];
		key[0..16].copy_from_slice(&edge_id.to_be_bytes());
		key[16..32].copy_from_slice(&node_id.to_be_bytes());
		key
	}

	/// Unpacks the 32 byte array into an (edge_id, node_id) tuple of u128s.
	///
	/// Returns (edge_id, node_id)
	#[inline(always)]
	// Uses Type Aliases for clarity
	pub fn unpack_adj_edge_data(data: &[u8]) -> Result<(EdgeId, NodeId), EngineError> {
		let edge_id = u128::from_be_bytes(
			data[0..16]
				.try_into()
				.map_err(|_| StorageError::SliceLengthError)?,
		);
		let node_id = u128::from_be_bytes(
			data[16..32]
				.try_into()
				.map_err(|_| StorageError::SliceLengthError)?,
		);
		Ok((edge_id, node_id))
	}

	/// Deletes all incident edges for a node/vector and cleans up adjacency indices.
	fn drop_incident_edges(&self, txn: &mut RwTxn, id: &u128) -> Result<(), EngineError> {
		let mut edges = HashSet::new();
		let mut out_edges = HashSet::new();
		let mut in_edges = HashSet::new();

		let mut other_out_edges = Vec::new();
		let mut other_in_edges = Vec::new();

		let iter = self.out_edges_db.prefix_iter(txn, &id.to_be_bytes())?;
		for result in iter {
			let (key, value) = result?;
			assert_eq!(key.len(), 20);
			let mut label = [0u8; 4];
			label.copy_from_slice(&key[16..20]);
			let (edge_id, to_node_id) = Self::unpack_adj_edge_data(value)?;
			edges.insert(edge_id);
			out_edges.insert(label);
			other_in_edges.push((to_node_id, label, edge_id));
		}

		let iter = self.in_edges_db.prefix_iter(txn, &id.to_be_bytes())?;
		for result in iter {
			let (key, value) = result?;
			assert_eq!(key.len(), 20);
			let mut label = [0u8; 4];
			label.copy_from_slice(&key[16..20]);
			let (edge_id, from_node_id) = Self::unpack_adj_edge_data(value)?;
			in_edges.insert(label);
			edges.insert(edge_id);
			other_out_edges.push((from_node_id, label, edge_id));
		}

		for edge in edges {
			self.edges_db.delete(txn, Self::edge_key(&edge))?;
		}
		for label_bytes in out_edges.iter() {
			self.out_edges_db
				.delete(txn, &Self::out_edge_key(id, label_bytes))?;
		}
		for label_bytes in in_edges.iter() {
			self.in_edges_db
				.delete(txn, &Self::in_edge_key(id, label_bytes))?;
		}

		for (other_node_id, label_bytes, edge_id) in other_out_edges {
			self.out_edges_db.delete_one_duplicate(
				txn,
				&Self::out_edge_key(&other_node_id, &label_bytes),
				&Self::pack_edge_data(&edge_id, id),
			)?;
		}
		for (other_node_id, label_bytes, edge_id) in other_in_edges {
			self.in_edges_db.delete_one_duplicate(
				txn,
				&Self::in_edge_key(&other_node_id, &label_bytes),
				&Self::pack_edge_data(&edge_id, id),
			)?;
		}

		Ok(())
	}
}

impl StorageConfig {
	pub fn new(
		schema: Option<String>,
		graphvis_node_label: Option<String>,
		embedding_model: Option<String>,
	) -> StorageConfig {
		Self {
			schema,
			graphvis_node_label,
			embedding_model,
		}
	}
}

impl DBMethods for HelixGraphStorage {
	/// Creates a secondary index lmdb db (table) for a given index name.
	///
	/// Accepts `SecondaryIndex` from the schema layer and converts to
	/// `ActiveSecondaryIndex` internally, filtering out `None`.
	fn create_secondary_index(&mut self, index: SecondaryIndex) -> Result<(), EngineError> {
		let active = index.into_active().ok_or_else(|| {
			StorageError::Backend(
				"cannot create a secondary index from SecondaryIndex::None".into(),
			)
		})?;
		let mut wtxn = self.graph_env.write_txn()?;
		match &active {
			ActiveSecondaryIndex::Unique(name) => {
				let db = self.graph_env.create_database(&mut wtxn, Some(name))?;
				wtxn.commit()?;
				self.secondary_indices.insert(name.clone(), (db, active));
			}
			ActiveSecondaryIndex::Index(name) => {
				let db = self.graph_env.create_database(&mut wtxn, Some(name))?;
				wtxn.commit()?;
				self.secondary_indices.insert(name.clone(), (db, active));
			}
		}
		Ok(())
	}

	/// Drops a secondary index lmdb db (table) for a given index name
	fn drop_secondary_index(&mut self, name: &str) -> Result<(), EngineError> {
		let mut wtxn = self.graph_env.write_txn()?;
		let (db, _) = self
			.secondary_indices
			.get(name)
			.ok_or_else(|| StorageError::Backend(format!("Secondary Index {name} not found")))?;
		db.clear(&mut wtxn)?;
		wtxn.commit()?;
		self.secondary_indices.remove(name);
		Ok(())
	}
}

impl StorageMethods for HelixGraphStorage {
	#[inline]
	fn get_node<'arena>(
		&self,
		txn: &RoTxn,
		id: &u128,
		arena: &'arena bumpalo::Bump,
	) -> Result<Node<'arena>, EngineError> {
		let node = match self.nodes_db.get(txn, Self::node_key(id))? {
			Some(data) => data,
			None => return Err(TraversalError::NodeNotFound.into()),
		};
		let node: Node = Node::from_bytes(*id, node, arena)?;
		let node = self.version_info.upgrade_to_node_latest(node);
		Ok(node)
	}

	#[inline]
	fn get_edge<'arena>(
		&self,
		txn: &RoTxn,
		id: &u128,
		arena: &'arena bumpalo::Bump,
	) -> Result<Edge<'arena>, EngineError> {
		let edge = match self.edges_db.get(txn, Self::edge_key(id))? {
			Some(data) => data,
			None => return Err(TraversalError::EdgeNotFound.into()),
		};
		let edge: Edge = Edge::from_bytes(*id, edge, arena)?;
		Ok(self.version_info.upgrade_to_edge_latest(edge))
	}

	fn drop_node(&self, txn: &mut RwTxn, id: &u128) -> Result<(), EngineError> {
		let arena = bumpalo::Bump::new();
		self.drop_incident_edges(txn, id)?;

		// delete secondary indices
		let node = self.get_node(txn, id, &arena)?;
		for (index_name, (db, _)) in &self.secondary_indices {
			// Use get_property like we do when adding, to handle id, label, and regular properties consistently
			match node.get_property(index_name) {
				Some(value) => match postcard::to_stdvec(value) {
					Ok(serialized) => {
						if let Err(e) = db.delete_one_duplicate(txn, &serialized, &node.id) {
							return Err(EngineError::from(e));
						}
					}
					Err(e) => return Err(EngineError::from(e)),
				},
				None => {
					// Property not found - this is expected for some indices
					// Continue to next index
				}
			}
		}

		// Delete node data and label
		self.nodes_db.delete(txn, Self::node_key(id))?;

		Ok(())
	}

	fn drop_edge(&self, txn: &mut RwTxn, edge_id: &u128) -> Result<(), EngineError> {
		let arena = bumpalo::Bump::new();
		// Get edge data first
		let edge_data = match self.edges_db.get(txn, Self::edge_key(edge_id))? {
			Some(data) => data,
			None => return Err(TraversalError::EdgeNotFound.into()),
		};
		let edge: Edge = Edge::from_bytes(*edge_id, edge_data, &arena)?;
		let label_hash = hash_label(edge.label, None);
		let out_edge_value = Self::pack_edge_data(edge_id, &edge.to_node);
		let in_edge_value = Self::pack_edge_data(edge_id, &edge.from_node);
		// Delete all edge-related data
		self.edges_db.delete(txn, Self::edge_key(edge_id))?;
		self.out_edges_db.delete_one_duplicate(
			txn,
			&Self::out_edge_key(&edge.from_node, &label_hash),
			&out_edge_value,
		)?;
		self.in_edges_db.delete_one_duplicate(
			txn,
			&Self::in_edge_key(&edge.to_node, &label_hash),
			&in_edge_value,
		)?;

		Ok(())
	}

	fn drop_vector(&self, txn: &mut RwTxn, id: &u128) -> Result<(), EngineError> {
		let arena = bumpalo::Bump::new();
		self.drop_incident_edges(txn, id)?;

		// Delete vector data
		self.vectors.delete(txn, *id, &arena)?;

		Ok(())
	}
}
