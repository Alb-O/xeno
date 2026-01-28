//! Built-in text object implementations.

use xeno_primitives::range::Range;

use crate::text_object;

text_object!(word, {
	trigger: 'w',
	description: "Word",
}, {
	inner: |_text, pos| {
		Some(Range::point(pos)) // Dummy implementation for now
	},
	around: |_text, pos| {
		Some(Range::point(pos)) // Dummy implementation for now
	}
});

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_text_object(&OBJ_word);
}
