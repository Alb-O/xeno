//! Document-level cursor movement (start, end).

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Range};

use crate::movement::make_range;

/// Move to document start.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, 0 as CharIdx, extend)
}

/// Move to document end.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, text.len_chars() as CharIdx, extend)
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
}
