use crate::helix_engine::storage_core::storage_methods::StorageMethods;
use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;
pub trait FromNAdapter<'db, 'arena, 'txn, I>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	/// Returns an iterator containing the nodes that the edges in `self.inner` originate from.
	fn from_n(
		self,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	FromNAdapter<'db, 'arena, 'txn, I> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline(always)]
	fn from_n(
		self,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	> {
		let iter = self.inner.filter_map(move |item| {
			if let Ok(TraversalValue::Edge(item)) = item {
				match self.storage.get_node(self.txn, &item.from_node, self.arena) {
					Ok(node) => Some(Ok(TraversalValue::Node(node))),
					Err(e) => Some(Err(e)),
				}
			} else {
				None
			}
		});
		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: iter,
		}
	}
}
