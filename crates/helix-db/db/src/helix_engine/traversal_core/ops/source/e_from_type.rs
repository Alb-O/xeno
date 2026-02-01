use heed3::byteorder::BE;
use heed3::types::{Bytes, U128};

use crate::helix_engine::traversal_core::decode_postcard_str_prefix;
use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, StorageError};
use crate::utils::items::Edge;

pub struct EFromType<'arena, 'txn, 's>
where
	'arena: 'txn,
{
	pub arena: &'arena bumpalo::Bump,
	pub iter: heed3::RoIter<'txn, U128<BE>, heed3::types::LazyDecode<Bytes>>,
	pub label: &'s [u8],
}

impl<'arena, 'txn, 's> Iterator for EFromType<'arena, 'txn, 's> {
	type Item = Result<TraversalValue<'arena>, EngineError>;

	fn next(&mut self) -> Option<Self::Item> {
		for value in self.iter.by_ref() {
			let (id, value) = value.unwrap();

			match value.decode() {
				Ok(value) => {
					let Some((label_in_lmdb, _)) = decode_postcard_str_prefix(value) else {
						continue;
					};

					if label_in_lmdb == self.label {
						match Edge::<'arena>::from_bytes(id, value, self.arena) {
							Ok(edge) => {
								return Some(Ok(TraversalValue::Edge(edge)));
							}
							Err(e) => {
								tracing::warn!(?e, edge_id = %id, "error decoding edge");
								return Some(Err(StorageError::Conversion(e.to_string()).into()));
							}
						}
					} else {
						continue;
					}
				}
				Err(e) => return Some(Err(StorageError::Conversion(e.to_string()).into())),
			}
		}
		None
	}
}
pub trait EFromTypeAdapter<'db, 'arena, 'txn, 's>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	fn e_from_type(
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
	EFromTypeAdapter<'db, 'arena, 'txn, 's> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline]
	fn e_from_type(
		self,
		label: &'s str,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	> {
		let iter = self
			.storage
			.edges_db
			.lazily_decode_data()
			.iter(self.txn)
			.unwrap();
		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: EFromType {
				arena: self.arena,
				iter,
				label: label.as_bytes(),
			},
		}
	}
}
