use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{TEXT_OBJECTS, TextObjectDef};
use crate::movement::select_surround_object;
use crate::range::Range;

fn parens_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '(', ')', true)
}

fn parens_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '(', ')', false)
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_PARENS: TextObjectDef = TextObjectDef {
	name: "parentheses",
	trigger: 'b',
	alt_triggers: &['(', ')'],
	description: "Select parentheses block",
	inner: parens_inner,
	around: parens_around,
};

fn braces_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '{', '}', true)
}

fn braces_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '{', '}', false)
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_BRACES: TextObjectDef = TextObjectDef {
	name: "braces",
	trigger: 'B',
	alt_triggers: &['{', '}'],
	description: "Select braces block",
	inner: braces_inner,
	around: braces_around,
};

fn brackets_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '[', ']', true)
}

fn brackets_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '[', ']', false)
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_BRACKETS: TextObjectDef = TextObjectDef {
	name: "brackets",
	trigger: 'r',
	alt_triggers: &['[', ']'],
	description: "Select brackets block",
	inner: brackets_inner,
	around: brackets_around,
};

fn angle_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '<', '>', true)
}

fn angle_around(text: RopeSlice, pos: usize) -> Option<Range> {
	select_surround_object(text, Range::point(pos), '<', '>', false)
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_ANGLE: TextObjectDef = TextObjectDef {
	name: "angle_brackets",
	trigger: 'a',
	alt_triggers: &['<', '>'],
	description: "Select angle brackets block",
	inner: angle_inner,
	around: angle_around,
};
