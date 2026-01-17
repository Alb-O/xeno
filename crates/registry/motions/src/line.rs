//! Line-based cursor movement (start, end, first non-whitespace).

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Range};

use crate::movement::make_range;

/// Moves the cursor to the start of the current line.
pub fn move_to_line_start(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line_idx = text.char_to_line(range.head);
	let start_pos: CharIdx = text.line_to_char(line_idx);
	make_range(range, start_pos, extend)
}

/// Moves the cursor to the end of the current line.
pub fn move_to_line_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line_idx = text.char_to_line(range.head);
	let start_pos = text.line_to_char(line_idx);
	let line_len = text.line(line_idx).len_chars();

	let is_last_line = line_idx == text.len_lines().saturating_sub(1);
	let end_pos: CharIdx = if is_last_line {
		start_pos + line_len
	} else {
		start_pos + line_len.saturating_sub(1)
	};

	make_range(range, end_pos, extend)
}

/// Moves the cursor to the first non-whitespace character on the current line.
pub fn move_to_first_nonwhitespace(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line_idx = text.char_to_line(range.head);
	let start_pos = text.line_to_char(line_idx);
	let line_text = text.line(line_idx);

	let mut first_non_ws: CharIdx = start_pos;
	for (i, ch) in line_text.chars().enumerate() {
		if !ch.is_whitespace() {
			first_non_ws = start_pos + i;
			break;
		}
	}

	make_range(range, first_non_ws, extend)
}

motion!(
	line_start,
	{ description: "Move to line start" },
	|text, range, _count, extend| move_to_line_start(text, range, extend)
);

motion!(
	line_end,
	{ description: "Move to line end" },
	|text, range, _count, extend| move_to_line_end(text, range, extend)
);

motion!(
	first_nonwhitespace,
	{ description: "Move to first non-whitespace" },
	|text, range, _count, extend| move_to_first_nonwhitespace(text, range, extend)
);

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

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
}
