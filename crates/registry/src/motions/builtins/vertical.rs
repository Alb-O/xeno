//! Vertical cursor movement (up, down).

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Direction, Range};
use xeno_primitives::visible_line_count;

use crate::motions::movement::make_range;

/// Moves the cursor vertically by the given number of lines.
pub fn move_vertically(
	text: RopeSlice,
	range: Range,
	direction: Direction,
	count: usize,
	extend: bool,
) -> Range {
	let pos: CharIdx = range.head;
	let line = text.char_to_line(pos);
	let line_start = text.line_to_char(line);
	let col = pos - line_start;

	let new_line = match direction {
		Direction::Forward => (line + count).min(visible_line_count(text).saturating_sub(1)),
		Direction::Backward => line.saturating_sub(count),
	};

	let new_line_start = text.line_to_char(new_line);
	let new_line_len = text.line(new_line).len_chars();
	let line_end_offset = new_line_len.saturating_sub(1);

	let new_col = col.min(line_end_offset);
	let new_pos: CharIdx = new_line_start + new_col;

	make_range(range, new_pos, extend)
}

motion!(up, { description: "Move up" }, |text, range, count, extend| {
	move_vertically(text, range, Direction::Backward, count, extend)
});

motion!(down, { description: "Move down" }, |text, range, count, extend| {
	move_vertically(text, range, Direction::Forward, count, extend)
});

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_move_down() {
		let text = Rope::from("hello\nworld\n");
		let slice = text.slice(..);
		let range = Range::point(2);

		let moved = move_vertically(slice, range, Direction::Forward, 1, false);
		assert_eq!(moved.head, 8);
	}

	#[test]
	fn test_move_up() {
		let text = Rope::from("hello\nworld\n");
		let slice = text.slice(..);
		let range = Range::point(8);

		let moved = move_vertically(slice, range, Direction::Backward, 1, false);
		assert_eq!(moved.head, 2);
	}
}
