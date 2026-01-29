//! Document movement logic.

use ropey::RopeSlice;
use xeno_primitives::max_cursor_pos;
use xeno_primitives::range::Range;

use super::make_range;

/// Move to document start.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, 0, extend)
}

/// Move to document end.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, max_cursor_pos(text), extend)
}
