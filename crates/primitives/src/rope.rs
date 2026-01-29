//! Rope utilities and extensions.

use ropey::RopeSlice;

use crate::range::CharIdx;

/// Returns the number of lines, including the empty line after a trailing newline.
#[inline]
pub fn visible_line_count(text: RopeSlice) -> usize {
	text.len_lines()
}

/// Returns the maximum valid cursor position, which is always the character count.
///
/// This allows the cursor to sit at the end of the final line, even if it is empty.
/// This is used for "gap-space" coordinates (e.g. insertion points).
#[inline]
pub fn max_cursor_pos(text: RopeSlice) -> CharIdx {
	text.len_chars()
}

/// Returns the maximum valid cell index.
///
/// A cell index is a position that points TO a character.
/// Returns None if the document is empty.
#[inline]
pub fn max_cell_pos(text: RopeSlice) -> Option<CharIdx> {
	let len = text.len_chars();
	if len > 0 { Some(len - 1) } else { None }
}

/// Clamps a position to a valid cell index.
///
/// If the document is empty, returns 0.
#[inline]
pub fn clamp_to_cell(pos: CharIdx, text: RopeSlice) -> CharIdx {
	if let Some(max) = max_cell_pos(text) {
		pos.min(max)
	} else {
		0
	}
}

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_no_trailing_newline() {
		let text = Rope::from("hello\nworld");
		assert_eq!(text.len_lines(), 2);
		assert_eq!(visible_line_count(text.slice(..)), 2);
	}

	#[test]
	fn test_trailing_newline() {
		let text = Rope::from("hello\nworld\n");
		assert_eq!(text.len_lines(), 3);
		assert_eq!(visible_line_count(text.slice(..)), 3);
	}

	#[test]
	fn test_single_line_no_newline() {
		let text = Rope::from("hello");
		assert_eq!(text.len_lines(), 1);
		assert_eq!(visible_line_count(text.slice(..)), 1);
	}

	#[test]
	fn test_single_line_with_newline() {
		let text = Rope::from("hello\n");
		assert_eq!(text.len_lines(), 2);
		assert_eq!(visible_line_count(text.slice(..)), 2);
	}

	#[test]
	fn test_empty() {
		let text = Rope::from("");
		assert_eq!(text.len_lines(), 1);
		assert_eq!(visible_line_count(text.slice(..)), 1);
	}

	#[test]
	fn test_only_newline() {
		let text = Rope::from("\n");
		assert_eq!(text.len_lines(), 2);
		assert_eq!(visible_line_count(text.slice(..)), 2);
	}
}
