use crate::text_object_handler;

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
