use crate::text_object_handler;

text_object_handler!(word, {
	inner: |_text, pos| {
		Some(xeno_primitives::Range::point(pos)) // Dummy implementation for now
	},
	around: |_text, pos| {
		Some(xeno_primitives::Range::point(pos)) // Dummy implementation for now
	},
});
