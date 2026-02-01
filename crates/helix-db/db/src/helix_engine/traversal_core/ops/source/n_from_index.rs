use serde::Serialize;

use crate::helix_engine::traversal_core::decode_postcard_str_prefix;
use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, StorageError};
use crate::protocol::value::Value;
use crate::utils::items::Node;

pub trait NFromIndexAdapter<'db, 'arena, 'txn, 's, K: Into<Value> + Serialize>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	/// Returns a new iterator that will return the node from the secondary index.
	///
	/// # Arguments
	///
	/// * `index` - The name of the secondary index.
	/// * `key` - The key to search for in the secondary index.
	///
	/// Note that both the `index` and `key` must be provided.
	/// The index must be a valid and existing secondary index and the key should match the type of the index.
	fn n_from_index(
		self,
		label: &'s str,
		index: &'s str,
		key: &'s K,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		K: Into<Value> + Serialize + Clone;
}

impl<
	'db,
	'arena,
	'txn,
	's,
	K: Into<Value> + Serialize,
	I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
> NFromIndexAdapter<'db, 'arena, 'txn, 's, K> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline]
	fn n_from_index(
		self,
		label: &'s str,
		index: &'s str,
		key: &K,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		K: Into<Value> + Serialize + Clone,
	{
		let (db, _) = self
			.storage
			.secondary_indices
			.get(index)
			.ok_or_else(|| StorageError::Backend(format!("Secondary Index {index} not found")))
			.unwrap();
		let label_as_bytes = label.as_bytes();
		let res = db
			.prefix_iter(self.txn, &postcard::to_stdvec(&Value::from(key)).unwrap())
			.unwrap()
			.filter_map(move |item| {
				if let Ok((_, node_id)) = item
					&& let Some(value) = self.storage.nodes_db.get(self.txn, &node_id).ok()?
				{
					let (label_in_lmdb, _) = decode_postcard_str_prefix(value)?;

					if label_in_lmdb == label_as_bytes {
						match Node::<'arena>::from_bytes(node_id, value, self.arena) {
							Ok(node) => {
								return Some(Ok(TraversalValue::Node(node)));
							}
							Err(e) => {
								tracing::warn!(?e, node_id = %node_id, "error decoding node");
								return Some(Err(StorageError::Conversion(e.to_string()).into()));
							}
						}
					} else {
						return None;
					}
				}
				None
			});

		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: res,
		}
	}
}
