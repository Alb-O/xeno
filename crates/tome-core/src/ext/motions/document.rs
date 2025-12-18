use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{MOTIONS, MotionDef};
use crate::movement::{self, move_to_document_end, move_to_document_start};
use crate::range::Range;

fn document_start(text: RopeSlice, range: Range, _count: usize, extend: bool) -> Range {
	move_to_document_start(text, range, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_DOCUMENT_START: MotionDef = MotionDef {
	name: "document_start",
	description: "Move to document start",
	handler: document_start,
};

fn document_end(text: RopeSlice, range: Range, _count: usize, extend: bool) -> Range {
	move_to_document_end(text, range, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_DOCUMENT_END: MotionDef = MotionDef {
	name: "document_end",
	description: "Move to document end",
	handler: document_end,
};

fn find_char_forward(_text: RopeSlice, range: Range, _count: usize, extend: bool) -> Range {
	movement::make_range(range, range.head, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_FIND_CHAR_FORWARD: MotionDef = MotionDef {
	name: "find_char_forward",
	description: "Find character forward (placeholder)",
	handler: find_char_forward,
};
