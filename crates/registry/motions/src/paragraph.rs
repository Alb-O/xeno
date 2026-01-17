//! Paragraph-based cursor movement.

use ropey::RopeSlice;
use xeno_primitives::range::Range;

use crate::movement::make_range;

/// Returns true if the line at `line_idx` is empty or contains only whitespace.
fn is_blank_line(text: RopeSlice, line_idx: usize) -> bool {
	let line = text.line(line_idx);
	line.chars().all(|c| c.is_whitespace())
}

/// Moves to the next paragraph boundary.
///
/// A paragraph is a contiguous block of non-blank lines. This function skips
/// forward past the current paragraph's remaining lines, then past any blank
/// lines, landing on the first line of the next paragraph.
pub fn move_to_next_paragraph(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	let total_lines = text.len_lines();
	if total_lines == 0 {
		return range;
	}

	let mut line = text.char_to_line(range.head);
	for _ in 0..count {
		if line >= total_lines.saturating_sub(1) {
			break;
		}
		while line < total_lines.saturating_sub(1) && !is_blank_line(text, line) {
			line += 1;
		}
		while line < total_lines.saturating_sub(1) && is_blank_line(text, line) {
			line += 1;
		}
	}

	make_range(range, text.line_to_char(line), extend)
}

/// Moves to the previous paragraph boundary.
///
/// A paragraph is a contiguous block of non-blank lines. This function moves
/// backwards past the current paragraph, past any blank lines, then finds
/// the start of the preceding paragraph.
pub fn move_to_prev_paragraph(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	let total_lines = text.len_lines();
	if total_lines == 0 {
		return range;
	}

	let mut line = text.char_to_line(range.head);
	for _ in 0..count {
		if line == 0 {
			break;
		}
		line -= 1;
		while line > 0 && !is_blank_line(text, line) {
			line -= 1;
		}
		while line > 0 && is_blank_line(text, line) {
			line -= 1;
		}
		while line > 0 && !is_blank_line(text, line - 1) {
			line -= 1;
		}
	}

	make_range(range, text.line_to_char(line), extend)
}

motion!(
	next_paragraph,
	{ description: "Move to next paragraph" },
	|text, range, count, extend| move_to_next_paragraph(text, range, count, extend)
);

motion!(
	prev_paragraph,
	{ description: "Move to previous paragraph" },
	|text, range, count, extend| move_to_prev_paragraph(text, range, count, extend)
);

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_next_paragraph() {
		let text = Rope::from("line1\nline2\n\nline3\nline4");
		let slice = text.slice(..);

		let moved = move_to_next_paragraph(slice, Range::point(0), 1, false);
		assert_eq!(moved.head, 13);

		let moved = move_to_next_paragraph(slice, Range::point(6), 1, false);
		assert_eq!(moved.head, 13);
	}

	#[test]
	fn test_prev_paragraph() {
		let text = Rope::from("line1\nline2\n\nline3\nline4");
		let slice = text.slice(..);

		let moved = move_to_prev_paragraph(slice, Range::point(13), 1, false);
		assert_eq!(moved.head, 0);

		let moved = move_to_prev_paragraph(slice, Range::point(19), 1, false);
		assert_eq!(moved.head, 0);
	}

	#[test]
	fn test_paragraph_with_multiple_blank_lines() {
		let text = Rope::from("para1\n\n\npara2");
		let slice = text.slice(..);

		let moved = move_to_next_paragraph(slice, Range::point(0), 1, false);
		assert_eq!(moved.head, 8);
	}

	#[test]
	fn test_paragraph_count() {
		let text = Rope::from("p1\n\np2\n\np3");
		let slice = text.slice(..);

		let moved = move_to_next_paragraph(slice, Range::point(0), 2, false);
		assert_eq!(moved.head, 8);
	}
}
