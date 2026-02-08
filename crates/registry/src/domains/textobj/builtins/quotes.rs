use crate::text_object_handler;

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
