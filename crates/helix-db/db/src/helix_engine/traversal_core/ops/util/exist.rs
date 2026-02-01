use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;

pub struct Exist<I> {
	pub iter: I,
}

impl<'arena, I> Exist<I>
where
	I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
{
	pub fn exists(iter: &mut I) -> bool {
		for item in iter.by_ref() {
			match item {
				Ok(_) => return true,
				Err(_) => continue,
			}
		}
		false
	}
}
