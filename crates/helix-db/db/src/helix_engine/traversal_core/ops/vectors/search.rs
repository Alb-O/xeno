use std::iter::once;

use heed3::RoTxn;

use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;
use crate::helix_engine::vector_core::hnsw::HNSW;
use crate::helix_engine::vector_core::vector::HVector;

pub trait SearchVAdapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	fn search_v<F, K>(
		self,
		query: &'arena [f64],
		k: K,
		label: &'arena str,
		filter: Option<&'arena [F]>,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: Fn(&HVector, &RoTxn) -> bool,
		K: TryInto<usize>,
		K::Error: std::fmt::Debug;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	SearchVAdapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	fn search_v<F, K>(
		self,
		query: &'arena [f64],
		k: K,
		label: &'arena str,
		filter: Option<&'arena [F]>,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: Fn(&HVector, &RoTxn) -> bool,
		K: TryInto<usize>,
		K::Error: std::fmt::Debug,
	{
		let vectors = self.storage.vectors.search(
			self.txn,
			query,
			k.try_into().unwrap(),
			label,
			filter,
			false,
			self.arena,
		);

		let iter = match vectors {
			Ok(vectors) => vectors
				.into_iter()
				.map(|vector| Ok::<TraversalValue, EngineError>(TraversalValue::Vector(vector)))
				.collect::<Vec<_>>()
				.into_iter(),
			Err(err) => once(Err(err.into())).collect::<Vec<_>>().into_iter(),
		};

		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: iter,
		}
	}
}
