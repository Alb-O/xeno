use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{MOTIONS, MotionDef};
use crate::movement::{move_to_first_nonwhitespace, move_to_line_end, move_to_line_start};
use crate::range::Range;

fn line_start(text: RopeSlice, range: Range, _count: usize, extend: bool) -> Range {
	move_to_line_start(text, range, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_LINE_START: MotionDef = MotionDef {
	name: "line_start",
	description: "Move to line start",
	handler: line_start,
};

fn line_end(text: RopeSlice, range: Range, _count: usize, extend: bool) -> Range {
	move_to_line_end(text, range, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_LINE_END: MotionDef = MotionDef {
	name: "line_end",
	description: "Move to line end",
	handler: line_end,
};

fn first_nonwhitespace(text: RopeSlice, range: Range, _count: usize, extend: bool) -> Range {
	move_to_first_nonwhitespace(text, range, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_FIRST_NONWHITESPACE: MotionDef = MotionDef {
	name: "first_nonwhitespace",
	description: "Move to first non-whitespace",
	handler: first_nonwhitespace,
};
