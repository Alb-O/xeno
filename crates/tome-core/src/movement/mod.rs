//! Movement functions for cursor and selection manipulation.
//!
//! This module is organized into submodules:
//! - `word`: Word movement (w, b, e commands)
//! - `find`: Character find (f, t, F, T commands)
//! - `objects`: Text object selection (inner/around word, surround)

mod find;
mod objects;
mod search;
mod word;

pub use find::{find_char_backward, find_char_forward};
pub use objects::{select_surround_object, select_word_object};
use ropey::RopeSlice;
pub use search::{escape_pattern, find_all_matches, find_next, find_prev, matches_pattern};
pub use word::{move_to_next_word_end, move_to_next_word_start, move_to_prev_word_start};

use crate::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use crate::range::{Direction, Range};

/// Word type for word movements (Kakoune style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordType {
	/// A word is alphanumeric characters (and those in extra_word_chars).
	Word,
	/// A WORD is any non-whitespace characters.
	WORD,
}

pub(crate) fn is_word_char(c: char) -> bool {
	c.is_alphanumeric() || c == '_'
}

/// Make a range for cursor movement - anchor stays, only head moves.
///
/// If `extend` is false, this performs a "move": the range collapses to a single point at `new_head`.
/// If `extend` is true, this performs a "selection extension": the anchor remains fixed, and the head moves to `new_head`.
pub(crate) fn make_range(range: Range, new_head: usize, extend: bool) -> Range {
	if extend {
		// Extend: keep old anchor
		Range::new(range.anchor, new_head)
	} else {
		// Move: collapse to new position (cursor becomes point at new head)
		Range::point(new_head)
	}
}

/// Make a range for selection-creating motions - creates new selection from old head to new head.
/// With extend, keeps existing anchor. Without extend, anchor moves to old head position.
pub(crate) fn make_range_select(range: Range, new_head: usize, extend: bool) -> Range {
	if extend {
		Range::new(range.anchor, new_head)
	} else {
		// Selection-creating motion: anchor at previous head, creates selection to new position
		Range::new(range.head, new_head)
	}
}

pub fn move_horizontally(
	text: RopeSlice,
	range: Range,
	direction: Direction,
	count: usize,
	extend: bool,
) -> Range {
	let pos = range.head;
	let new_pos = match direction {
		Direction::Forward => {
			let mut p = pos;
			for _ in 0..count {
				p = next_grapheme_boundary(text, p);
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

pub fn move_vertically(
	text: RopeSlice,
	range: Range,
	direction: Direction,
	count: usize,
	extend: bool,
) -> Range {
	let pos = range.head;
	let line = text.char_to_line(pos);
	let line_start = text.line_to_char(line);
	let col = pos - line_start;

	let new_line = match direction {
		Direction::Forward => (line + count).min(text.len_lines().saturating_sub(1)),
		Direction::Backward => line.saturating_sub(count),
	};

	let new_line_start = text.line_to_char(new_line);
	let new_line_len = text.line(new_line).len_chars();
	let line_end_offset = if new_line == text.len_lines().saturating_sub(1) {
		new_line_len
	} else {
		new_line_len.saturating_sub(1)
	};

	let new_col = col.min(line_end_offset);
	let new_pos = new_line_start + new_col;

	make_range(range, new_pos, extend)
}

pub fn move_to_line_start(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	make_range(range, line_start, extend)
}

pub fn move_to_line_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	let line_len = text.line(line).len_chars();

	let is_last_line = line == text.len_lines().saturating_sub(1);
	let line_end = if is_last_line {
		line_start + line_len
	} else {
		line_start + line_len.saturating_sub(1)
	};

	make_range(range, line_end, extend)
}

pub fn move_to_first_nonwhitespace(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	let line_text = text.line(line);

	let mut first_non_ws = line_start;
	for (i, ch) in line_text.chars().enumerate() {
		if !ch.is_whitespace() {
			first_non_ws = line_start + i;
			break;
		}
	}

	make_range(range, first_non_ws, extend)
}

/// Move to document start.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, 0, extend)
}

/// Move to document end.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, text.len_chars(), extend)
}

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

	#[test]
	fn test_move_to_line_start() {
		let text = Rope::from("hello\nworld\n");
		let slice = text.slice(..);
		let range = Range::point(8);

		let moved = move_to_line_start(slice, range, false);
		assert_eq!(moved.head, 6);
	}

	#[test]
	fn test_move_to_line_end() {
		let text = Rope::from("hello\nworld\n");
		let slice = text.slice(..);
		let range = Range::point(6);

		let moved = move_to_line_end(slice, range, false);
		assert_eq!(moved.head, 11);
	}

	#[test]
	fn test_move_to_first_nonwhitespace() {
		let text = Rope::from("  hello\n");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_to_first_nonwhitespace(slice, range, false);
		assert_eq!(moved.head, 2);
	}

	#[test]
	fn test_document_movement() {
		let text = Rope::from("line1\nline2\nline3");
		let slice = text.slice(..);
		let range = Range::point(7);

		let start = move_to_document_start(slice, range, false);
		assert_eq!(start.head, 0);

		let end = move_to_document_end(slice, range, false);
		assert_eq!(end.head, 17);
	}
}
