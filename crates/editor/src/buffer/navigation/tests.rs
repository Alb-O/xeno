use super::*;
use crate::buffer::{Buffer, ViewId};

fn make_buffer(content: &str) -> Buffer {
	Buffer::new(ViewId(1), content.to_string(), None)
}

#[test]
fn goal_column_preserved_across_short_lines() {
	// Lines: "long line with text" / "" / "short" / "another long line here"
	let mut buffer = make_buffer("long line with text\n\nshort\nanother long line here");
	buffer.text_width = 80;
	buffer.cursor = 10;
	buffer.selection = xeno_primitives::Selection::point(10);

	// Move through empty line - snaps to col 0 but goal preserved
	buffer.move_visual_vertical(MoveDir::Forward, 1, false, 4);
	assert_eq!(buffer.cursor, 20);
	assert_eq!(buffer.goal_column, Some(10));

	// Move to "short" - clamps to newline (col 5) but goal preserved
	buffer.move_visual_vertical(MoveDir::Forward, 1, false, 4);
	assert_eq!(buffer.cursor, 26); // position of '\n' after "short"
	assert_eq!(buffer.goal_column, Some(10));

	// Move to long line - restores to col 10
	buffer.move_visual_vertical(MoveDir::Forward, 1, false, 4);
	assert_eq!(buffer.cursor, 37);
	assert_eq!(buffer.goal_column, Some(10));
}

#[test]
fn goal_column_reset_on_horizontal_movement() {
	let mut buffer = make_buffer("long line\nshort\nanother long line");
	buffer.text_width = 80;
	buffer.cursor = 5;
	buffer.selection = xeno_primitives::Selection::point(5);

	buffer.move_visual_vertical(MoveDir::Forward, 1, false, 4);
	assert_eq!(buffer.goal_column, Some(5));

	buffer.set_cursor(12);
	assert_eq!(buffer.goal_column, None);
}

#[test]
fn goal_column_set_from_current_position() {
	// Lines: "hello world" / "hi" / "longer line here"
	let mut buffer = make_buffer("hello world\nhi\nlonger line here");
	buffer.text_width = 80;
	buffer.cursor = 8;
	buffer.selection = xeno_primitives::Selection::point(8);
	assert_eq!(buffer.goal_column, None);

	// First vertical move sets goal from current col
	buffer.move_visual_vertical(MoveDir::Forward, 1, false, 4);
	assert_eq!(buffer.goal_column, Some(8));
	assert_eq!(buffer.cursor, 14); // position of '\n' after "hi"

	// Restore to col 8 on longer line
	buffer.move_visual_vertical(MoveDir::Forward, 1, false, 4);
	assert_eq!(buffer.cursor, 23);
}

#[test]
fn goal_column_preserved_moving_up() {
	// Lines: "another long line here" / "short" / "" / "long line with text"
	let mut buffer = make_buffer("another long line here\nshort\n\nlong line with text");
	buffer.text_width = 80;
	buffer.cursor = 45; // col 15 on last line
	buffer.selection = xeno_primitives::Selection::point(45);

	buffer.move_visual_vertical(MoveDir::Backward, 1, false, 4);
	assert_eq!(buffer.cursor, 29); // empty line
	assert_eq!(buffer.goal_column, Some(15));

	buffer.move_visual_vertical(MoveDir::Backward, 1, false, 4);
	assert_eq!(buffer.cursor, 28); // position of '\n' after "short"
	assert_eq!(buffer.goal_column, Some(15));

	buffer.move_visual_vertical(MoveDir::Backward, 1, false, 4);
	assert_eq!(buffer.cursor, 15); // restored to col 15
	assert_eq!(buffer.goal_column, Some(15));
}
