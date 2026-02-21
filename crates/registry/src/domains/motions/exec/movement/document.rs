//! Document movement logic.

use ropey::RopeSlice;
use xeno_primitives::Range;

use super::make_range;

/// Move to document start.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, 0, extend)
}

/// Move to document end.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	let pos = xeno_primitives::clamp_to_cell(text.len_chars(), text);
	make_range(range, pos, extend)
}
