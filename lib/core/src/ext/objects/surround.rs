use ropey::RopeSlice;

use crate::movement::select_surround_object;
use crate::range::Range;
use crate::text_object;

text_object!(
	parentheses,
	{
		trigger: 'b',
		alt_triggers: &['(', ')'],
		description: "Select parentheses block"
	},
	{
		inner: parens_inner,
		around: parens_around,
	}
);

fn parens_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '(', ')', true)
}

fn parens_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '(', ')', false)
}

text_object!(
	braces,
	{
		trigger: 'B',
		alt_triggers: &['{', '}'],
		description: "Select braces block"
	},
	{
		inner: braces_inner,
		around: braces_around,
	}
);

fn braces_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '{', '}', true)
}

fn braces_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '{', '}', false)
}

text_object!(
	brackets,
	{
		trigger: 'r',
		alt_triggers: &['[', ']'],
		description: "Select brackets block"
	},
	{
		inner: brackets_inner,
		around: brackets_around,
	}
);

fn brackets_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '[', ']', true)
}

fn brackets_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '[', ']', false)
}

text_object!(
	angle_brackets,
	{
		trigger: 'a',
		alt_triggers: &['<', '>'],
		description: "Select angle brackets block"
	},
	{
		inner: angle_inner,
		around: angle_around,
	}
);

fn angle_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '<', '>', true)
}

fn angle_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '<', '>', false)
}
