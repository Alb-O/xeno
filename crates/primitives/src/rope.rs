//! Rope utilities and extensions.

use ropey::RopeSlice;

use crate::range::CharIdx;

/// Returns the number of user-visible lines, excluding the phantom empty line
/// after a trailing newline.
///
/// Ropey counts `"hello\nworld\n"` as 3 lines (the empty line after the final
/// `\n`). This function returns 2, matching user expectations.
#[inline]
pub fn visible_line_count(text: RopeSlice) -> usize {
	let len = text.len_chars();
	if len > 0 && text.char(len - 1) == '\n' {
		text.len_lines() - 1
	} else {
		text.len_lines()
	}
}

/// Returns the maximum valid cursor position, excluding the phantom position
/// after a trailing newline.
///
/// For text ending with `\n`, the cursor should land on the final newline
/// character, not past it (which would be on the phantom empty line).
#[inline]
pub fn max_cursor_pos(text: RopeSlice) -> CharIdx {
	let len = text.len_chars();
	if len > 0 && text.char(len - 1) == '\n' {
		len - 1
	} else {
		len
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
		assert_eq!(visible_line_count(text.slice(..)), 2);
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
		assert_eq!(visible_line_count(text.slice(..)), 1);
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
		assert_eq!(visible_line_count(text.slice(..)), 1);
	}
}
