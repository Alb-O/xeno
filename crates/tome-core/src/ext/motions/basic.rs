use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{MOTIONS, MotionDef};
use crate::movement::move_horizontally;
use crate::range::{Direction, Range};

fn move_left(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_horizontally(text, range, Direction::Backward, count, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_LEFT: MotionDef = MotionDef {
	name: "move_left",
	description: "Move left",
	handler: move_left,
};

fn move_right(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	move_horizontally(text, range, Direction::Forward, count, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_RIGHT: MotionDef = MotionDef {
	name: "move_right",
	description: "Move right",
	handler: move_right,
};

fn move_up(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	crate::movement::move_vertically(text, range, Direction::Backward, count, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_UP: MotionDef = MotionDef {
	name: "move_up",
	description: "Move up",
	handler: move_up,
};

fn move_down(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	crate::movement::move_vertically(text, range, Direction::Forward, count, extend)
}

#[distributed_slice(MOTIONS)]
static MOTION_DOWN: MotionDef = MotionDef {
	name: "move_down",
	description: "Move down",
	handler: move_down,
};
