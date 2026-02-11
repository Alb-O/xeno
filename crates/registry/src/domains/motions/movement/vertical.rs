//! Vertical movement logic.

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Direction, Range};
use xeno_primitives::visible_line_count;

use super::make_range;

/// Moves the cursor vertically by the given number of lines.
pub fn move_vertically(text: RopeSlice, range: Range, direction: Direction, count: usize, extend: bool) -> Range {
	let pos: CharIdx = range.head;
	let line = text.char_to_line(pos);
	let line_start = text.line_to_char(line);
	let col = pos - line_start;

	let total_lines = visible_line_count(text);
	let new_line = match direction {
		Direction::Forward => (line + count).min(total_lines.saturating_sub(1)),
		Direction::Backward => line.saturating_sub(count),
	};

	let new_line_start = text.line_to_char(new_line);
	let new_line_content = text.line(new_line);
	let new_line_len = new_line_content.len_chars();
	let has_newline = new_line_len > 0 && new_line_content.char(new_line_len - 1) == '\n';
	let line_end_offset = if has_newline { new_line_len - 1 } else { new_line_len };

	let new_col = col.min(line_end_offset);
	let new_pos: CharIdx = new_line_start + new_col;

	make_range(range, new_pos, extend)
}
