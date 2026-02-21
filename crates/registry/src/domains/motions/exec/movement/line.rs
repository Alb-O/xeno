//! Line movement logic.

use ropey::RopeSlice;
use xeno_primitives::{CharIdx, Range};

use super::{LineBoundary, make_range};

pub fn move_to_line_boundary(text: RopeSlice, range: Range, boundary: LineBoundary, extend: bool) -> Range {
	match boundary {
		LineBoundary::Start => move_to_line_start(text, range, extend),
		LineBoundary::End => move_to_line_end(text, range, extend),
		LineBoundary::FirstNonBlank => move_to_first_nonwhitespace(text, range, extend),
	}
}

/// Moves the cursor to the start of the current line.
pub fn move_to_line_start(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start: CharIdx = text.line_to_char(line);
	make_range(range, line_start, extend)
}

/// Moves the cursor to the end of the current line.
pub fn move_to_line_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	let line_content = text.line(line);
	let line_len = line_content.len_chars();

	let line_end = if line_len > 0 {
		let has_newline = line_content.char(line_len - 1) == '\n';
		if has_newline && line_len > 1 {
			// Land on the last character before the newline
			line_start + line_len - 2
		} else {
			// Land on the last character (which might be the newline if it's the only char)
			line_start + line_len - 1
		}
	} else {
		line_start
	};

	make_range(range, line_end, extend)
}

/// Moves the cursor to the first non-whitespace character on the current line.
pub fn move_to_first_nonwhitespace(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	let line_text = text.line(line);

	let mut first_non_ws: CharIdx = line_start;
	for (i, ch) in line_text.chars().enumerate() {
		if !ch.is_whitespace() {
			first_non_ws = line_start + i;
			break;
		}
	}

	make_range(range, first_non_ws, extend)
}
