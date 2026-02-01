use crate::helix_engine::traversal_core::traversal_iter::{
	RoTraversalIterator, RwTraversalIterator,
};
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::GraphError;
use crate::protocol::value::Value;

pub trait CountAdapter<'arena>: Iterator {
	fn count_to_val(self) -> Value;
}

impl<'db, 'arena: 'txn, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, GraphError>>>
	CountAdapter<'arena> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	fn count_to_val(self) -> Value {
		Value::from(self.inner.count())
	}
}

impl<'db, 'arena: 'txn, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, GraphError>>>
	CountAdapter<'arena> for RwTraversalIterator<'db, 'arena, 'txn, I>
{
	fn count_to_val(self) -> Value {
		Value::from(self.inner.count())
	}
}
