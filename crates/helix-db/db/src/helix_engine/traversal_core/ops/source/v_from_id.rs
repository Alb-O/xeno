use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, VectorError};

pub trait VFromIdAdapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
where
	'db: 'arena,
	'arena: 'txn,
{
	/// Returns an iterator containing the vector with the given id.
	///
	/// Note that the `id` cannot be empty and must be a valid, existing vector id.
	fn v_from_id(
		self,
		id: &u128,
		get_vector_data: bool,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	VFromIdAdapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
where
	'db: 'arena,
	'arena: 'txn,
{
	#[inline]
	fn v_from_id(
		self,
		id: &u128,
		get_vector_data: bool,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	> {
		let vec = if get_vector_data {
			match self
				.storage
				.vectors
				.get_full_vector(self.txn, *id, self.arena)
			{
				Ok(vec) => {
					if vec.deleted {
						Err(EngineError::from(VectorError::VectorDeleted))
					} else {
						Ok(TraversalValue::Vector(vec))
					}
				}
				Err(e) => Err(EngineError::from(e)),
			}
		} else {
			match self
				.storage
				.vectors
				.get_vector_properties(self.txn, *id, self.arena)
			{
				Ok(Some(vec)) => {
					if vec.deleted {
						Err(EngineError::from(VectorError::VectorDeleted))
					} else {
						Ok(TraversalValue::VectorNodeWithoutVectorData(vec))
					}
				}
				Ok(None) => Err(EngineError::from(VectorError::VectorNotFound(
					id.to_string(),
				))),
				Err(e) => Err(EngineError::from(e)),
			}
		};

		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: std::iter::once(vec),
		}
	}
}
