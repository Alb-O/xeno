use crate::helix_engine::traversal_core::decode_postcard_str_prefix;
use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, StorageError};
use crate::utils::items::Node;

pub trait NFromTypeAdapter<'db, 'arena, 'txn, 's>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	/// Returns an iterator containing the nodes with the given label.
	///
	/// Scans all nodes and compares labels without full deserialization.
	/// The label is the first field in the postcard-serialized node data,
	/// encoded as a LEB128 varint length followed by raw UTF-8 bytes.
	fn n_from_type(
		self,
		label: &'s str,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>;
}
impl<'db, 'arena, 'txn, 's, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	NFromTypeAdapter<'db, 'arena, 'txn, 's> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline]
	fn n_from_type(
		self,
		label: &'s str,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	> {
		let label_as_bytes = label.as_bytes();
		let iter = self
			.storage
			.nodes_db
			.iter(self.txn)
			.unwrap()
			.filter_map(move |item| {
				if let Ok((id, value)) = item {
					let (label_in_lmdb, _) = decode_postcard_str_prefix(value)?;

					if label_in_lmdb == label_as_bytes {
						match Node::<'arena>::from_bytes(id, value, self.arena) {
							Ok(node) => {
								return Some(Ok(TraversalValue::Node(node)));
							}
							Err(e) => {
								tracing::warn!(?e, node_id = %id, "error decoding node");
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
			inner: iter,
		}
	}
}
