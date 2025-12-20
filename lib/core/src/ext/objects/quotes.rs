use ropey::RopeSlice;

use crate::movement::select_surround_object;
use crate::range::Range;
use crate::text_object;

text_object!(
	double_quotes,
	{
		trigger: '"',
		alt_triggers: &['Q'],
		description: "Select double-quoted string"
	},
	{
		inner: double_quotes_inner,
		around: double_quotes_around,
	}
);

fn double_quotes_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '"', '"', true)
}

fn double_quotes_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '"', '"', false)
}

text_object!(
	single_quotes,
	{
		trigger: '\'',
		alt_triggers: &['q'],
		description: "Select single-quoted string"
	},
	{
		inner: single_quotes_inner,
		around: single_quotes_around,
	}
);

fn single_quotes_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '\'', '\'', true)
}

fn single_quotes_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '\'', '\'', false)
}

text_object!(
	backticks,
	{
		trigger: '`',
		alt_triggers: &['g'],
		description: "Select backtick-quoted string"
	},
	{
		inner: backticks_inner,
		around: backticks_around,
	}
);

fn backticks_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '`', '`', true)
}

fn backticks_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '`', '`', false)
}
