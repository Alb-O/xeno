//! Paragraph movement logic.

use ropey::RopeSlice;
use xeno_primitives::range::{Direction, Range};

use super::make_range;

/// Moves the cursor to the next or previous paragraph.
pub fn move_paragraph(text: RopeSlice, range: Range, direction: Direction, count: usize, extend: bool) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let mut pos = range.head;

	for _ in 0..count {
		match direction {
			Direction::Forward => {
				// Skip current paragraph
				while pos < len && !is_empty_line(text, pos) {
					pos = next_line_start(text, pos);
				}
				// Skip empty lines
				while pos < len && is_empty_line(text, pos) {
					pos = next_line_start(text, pos);
				}
			}
			Direction::Backward => {
				// Move to previous line start
				if pos > 0 {
					pos = prev_line_start(text, pos);
				}
				// Skip current paragraph
				while pos > 0 && !is_empty_line(text, pos) {
					pos = prev_line_start(text, pos);
				}
				// Skip empty lines
				while pos > 0 && is_empty_line(text, pos) {
					pos = prev_line_start(text, pos);
				}
			}
		}
	}

	make_range(range, pos, extend)
}

fn is_empty_line(text: RopeSlice, pos: usize) -> bool {
	let line_idx = text.char_to_line(pos);
	let line = text.line(line_idx);
	line.len_chars() == 0 || (line.len_chars() == 1 && line.char(0) == '\n')
}

fn next_line_start(text: RopeSlice, pos: usize) -> usize {
	let line_idx = text.char_to_line(pos);
	if line_idx + 1 < text.len_lines() {
		text.line_to_char(line_idx + 1)
	} else {
		text.len_chars()
	}
}

fn prev_line_start(text: RopeSlice, pos: usize) -> usize {
	let line_idx = text.char_to_line(pos);
	if line_idx > 0 { text.line_to_char(line_idx - 1) } else { 0 }
}
