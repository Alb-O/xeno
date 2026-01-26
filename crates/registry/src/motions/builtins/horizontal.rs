//! Horizontal cursor movement (left, right).

use ropey::RopeSlice;
use xeno_primitives::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use xeno_primitives::max_cursor_pos;
use xeno_primitives::range::{CharIdx, Direction, Range};

use crate::motions::movement::make_range;

/// Moves the cursor horizontally by the given number of graphemes.
pub fn move_horizontally(
	text: RopeSlice,
	range: Range,
	direction: Direction,
	count: usize,
	extend: bool,
) -> Range {
	let pos: CharIdx = range.head;
	let max_pos = max_cursor_pos(text);
	let new_pos: CharIdx = match direction {
		Direction::Forward => {
			let mut p = pos;
			for _ in 0..count {
				let next = next_grapheme_boundary(text, p);
				if next > max_pos {
					break;
				}
				p = next;
			}
			p
		}
		Direction::Backward => {
			let mut p = pos;
			for _ in 0..count {
				p = prev_grapheme_boundary(text, p);
			}
			p
		}
	};

	make_range(range, new_pos, extend)
}

motion!(left, { description: "Move left" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Backward, count, extend)
});

motion!(right, { description: "Move right" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Forward, count, extend)
});

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_move_forward() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_horizontally(slice, range, Direction::Forward, 1, false);
		assert_eq!(moved.head, 1);
	}

	#[test]
	fn test_move_backward() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(5);

		let moved = move_horizontally(slice, range, Direction::Backward, 2, false);
		assert_eq!(moved.head, 3);
	}

	#[test]
	fn test_move_forward_extend() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_horizontally(slice, range, Direction::Forward, 5, true);
		assert_eq!(moved.anchor, 0);
		assert_eq!(moved.head, 5);
	}
}
