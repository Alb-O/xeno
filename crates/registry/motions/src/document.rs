//! Document-level cursor movement (start, end).

use ropey::RopeSlice;
use xeno_primitives::max_cursor_pos;
use xeno_primitives::range::{CharIdx, Range};

use crate::movement::make_range;

/// Moves cursor to start of document.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, 0 as CharIdx, extend)
}

/// Moves cursor to end of document, landing on the final newline if present.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, max_cursor_pos(text), extend)
}

motion!(
	document_start,
	{ description: "Move to document start" },
	|text, range, _count, extend| move_to_document_start(text, range, extend)
);

motion!(
	document_end,
	{ description: "Move to document end" },
	|text, range, _count, extend| move_to_document_end(text, range, extend)
);

motion!(
	find_char_forward,
	{ description: "Find character forward (placeholder)" },
	|_text, range, _count, extend| make_range(range, range.head, extend)
);

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

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

	#[test]
	fn test_document_end_with_trailing_newline() {
		let text = Rope::from("line1\nline2\n");
		let slice = text.slice(..);
		let range = Range::point(0);

		let end = move_to_document_end(slice, range, false);
		assert_eq!(end.head, 11);
		assert_eq!(slice.char(end.head), '\n');
	}
}
