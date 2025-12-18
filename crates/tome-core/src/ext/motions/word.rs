use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{MOTIONS, MotionDef};
use crate::movement::{
	WordType, move_to_next_word_end, move_to_next_word_start, move_to_prev_word_start,
};
use crate::range::Range;

fn next_word_start(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_to_next_word_start(text, range, count, WordType::Word, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_NEXT_WORD_START: MotionDef = MotionDef {
	name: "next_word_start",
	description: "Move to next word start",
	handler: next_word_start,
};

fn prev_word_start(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_to_prev_word_start(text, range, count, WordType::Word, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_PREV_WORD_START: MotionDef = MotionDef {
	name: "prev_word_start",
	description: "Move to previous word start",
	handler: prev_word_start,
};

fn next_word_end(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_to_next_word_end(text, range, count, WordType::Word, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_NEXT_WORD_END: MotionDef = MotionDef {
	name: "next_word_end",
	description: "Move to next word end",
	handler: next_word_end,
};

fn next_big_word_start(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_to_next_word_start(text, range, count, WordType::WORD, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_NEXT_BIG_WORD_START: MotionDef = MotionDef {
	name: "next_WORD_start",
	description: "Move to next WORD start",
	handler: next_big_word_start,
};

// Alias for action/binding names
#[distributed_slice(MOTIONS)]
static MOTION_NEXT_LONG_WORD_START: MotionDef = MotionDef {
	name: "next_long_word_start",
	description: "Move to next WORD start",
	handler: next_big_word_start,
};

fn prev_big_word_start(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_to_prev_word_start(text, range, count, WordType::WORD, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_PREV_BIG_WORD_START: MotionDef = MotionDef {
	name: "prev_WORD_start",
	description: "Move to previous WORD start",
	handler: prev_big_word_start,
};

#[distributed_slice(MOTIONS)]
static MOTION_PREV_LONG_WORD_START: MotionDef = MotionDef {
	name: "prev_long_word_start",
	description: "Move to previous WORD start",
	handler: prev_big_word_start,
};

fn next_big_word_end(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_to_next_word_end(text, range, count, WordType::WORD, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_NEXT_BIG_WORD_END: MotionDef = MotionDef {
	name: "next_WORD_end",
	description: "Move to next WORD end",
	handler: next_big_word_end,
};

#[distributed_slice(MOTIONS)]
static MOTION_NEXT_LONG_WORD_END: MotionDef = MotionDef {
	name: "next_long_word_end",
	description: "Move to next WORD end",
	handler: next_big_word_end,
};
