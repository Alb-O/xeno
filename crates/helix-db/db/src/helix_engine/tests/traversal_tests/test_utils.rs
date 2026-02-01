use bumpalo::Bump;

use crate::protocol::value::Value;
use crate::utils::properties::ImmutablePropertiesMap;

pub fn props_map<'arena>(
	arena: &'arena Bump,
	props: Vec<(String, Value)>,
) -> ImmutablePropertiesMap<'arena> {
	let len = props.len();
	ImmutablePropertiesMap::new(
		len,
		props.into_iter().map(|(key, value)| {
			let key: &'arena str = arena.alloc_str(&key);
			(key, value)
		}),
		arena,
	)
}

pub fn props_option<'arena>(
	arena: &'arena Bump,
	props: Vec<(String, Value)>,
) -> Option<ImmutablePropertiesMap<'arena>> {
	Some(props_map(arena, props))
}
