use heed3::RoTxn;

use crate::helix_engine::traversal_core::traversal_iter::RwTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;
use crate::helix_engine::vector_core::hnsw::HNSW;
use crate::helix_engine::vector_core::vector::HVector;
use crate::utils::properties::ImmutablePropertiesMap;

pub trait InsertVAdapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	fn insert_v<F>(
		self,
		query: &'arena [f64],
		label: &'arena str,
		properties: Option<ImmutablePropertiesMap<'arena>>,
	) -> RwTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: Fn(&HVector<'arena>, &RoTxn<'db>) -> bool;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	InsertVAdapter<'db, 'arena, 'txn> for RwTraversalIterator<'db, 'arena, 'txn, I>
{
	fn insert_v<F>(
		self,
		query: &'arena [f64],
		label: &'arena str,
		properties: Option<ImmutablePropertiesMap<'arena>>,
	) -> RwTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: Fn(&HVector<'arena>, &RoTxn<'db>) -> bool,
	{
		let vector: Result<HVector<'arena>, crate::helix_engine::types::VectorError> = self
			.storage
			.vectors
			.insert::<F>(self.txn, label, query, properties, self.arena);

		let result = match vector {
			Ok(vector) => Ok(TraversalValue::Vector(vector)),
			Err(e) => Err(EngineError::from(e)),
		};

		RwTraversalIterator {
			inner: std::iter::once(result),
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
		}
	}
}
