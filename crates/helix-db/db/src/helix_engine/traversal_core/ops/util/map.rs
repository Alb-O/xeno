use heed3::RoTxn;

use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;

pub struct Map<'db, 'txn, I, F> {
	iter: I,
	txn: &'txn RoTxn<'db>,
	f: F,
}

// implementing iterator for filter ref
impl<'db, 'arena, 'txn, I, F> Iterator for Map<'db, 'txn, I, F>
where
	I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	F: FnMut(TraversalValue<'arena>, &RoTxn<'db>) -> Result<TraversalValue<'arena>, EngineError>,
{
	type Item = I::Item;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(item) = self.iter.by_ref().next() {
			return match item {
				Ok(item) => Some((self.f)(item, self.txn)),
				Err(e) => return Some(Err(e)),
			};
		}
		None
	}
}

pub trait MapAdapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	/// MapTraversal maps the iterator by taking a reference
	/// to each item and a transaction.
	///
	/// # Arguments
	///
	/// * `f` - A function to map the iterator
	///
	/// # Example
	///
	/// ```rust
	/// let traversal = G::new(storage, &txn).map_traversal(|item, txn| {
	///     Ok(item)
	/// });
	/// ```
	fn map_traversal<F>(
		self,
		f: F,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: FnMut(
			TraversalValue<'arena>,
			&RoTxn<'db>,
		) -> Result<TraversalValue<'arena>, EngineError>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	MapAdapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline]
	fn map_traversal<F>(
		self,
		f: F,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: FnMut(
			TraversalValue<'arena>,
			&RoTxn<'db>,
		) -> Result<TraversalValue<'arena>, EngineError>,
	{
		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: Map {
				iter: self.inner,
				txn: self.txn,
				f,
			},
		}
	}
}
