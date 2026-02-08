//! Horizontal movement logic.

use ropey::RopeSlice;
use xeno_primitives::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use xeno_primitives::max_cursor_pos;
use xeno_primitives::range::{CharIdx, Direction, Range};

use super::make_range;

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
