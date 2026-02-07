//! Built-in text object implementations.

use crate::text_object_handler;

// ── Word ─────────────────────────────────────────────────────────────

text_object_handler!(word, {
	inner: |_text, pos| {
		Some(xeno_primitives::Range::point(pos)) // Dummy implementation for now
	},
	around: |_text, pos| {
		Some(xeno_primitives::Range::point(pos)) // Dummy implementation for now
	},
});

// ── Bracket pairs ────────────────────────────────────────────────────

text_object_handler!(parens, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'(',
			')',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'(',
			')',
			false,
		)
	},
});

text_object_handler!(brackets, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'[',
			']',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'[',
			']',
			false,
		)
	},
});

text_object_handler!(braces, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'{',
			'}',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'{',
			'}',
			false,
		)
	},
});

text_object_handler!(angle, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'<',
			'>',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'<',
			'>',
			false,
		)
	},
});

// ── Quote objects ────────────────────────────────────────────────────

text_object_handler!(quotes, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'"',
			'"',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'"',
			'"',
			false,
		)
	},
});

text_object_handler!(single_quotes, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'\'',
			'\'',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'\'',
			'\'',
			false,
		)
	},
});

text_object_handler!(backticks, {
	inner: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'`',
			'`',
			true,
		)
	},
	around: |text, pos| {
		crate::motions::movement::select_surround_object(
			text,
			xeno_primitives::Range::point(pos),
			'`',
			'`',
			false,
		)
	},
});

// ── Registration ─────────────────────────────────────────────────────

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	let metadata = crate::kdl::loader::load_text_object_metadata();
	let handlers = inventory::iter::<crate::textobj::TextObjectHandlerReg>
		.into_iter()
		.map(|r| r.0);
	let linked = crate::kdl::link::link_text_objects(&metadata, handlers);

	for def in linked {
		builder.register_linked_text_object(def);
	}
}
