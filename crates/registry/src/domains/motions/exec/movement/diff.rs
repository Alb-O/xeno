//! Diff movement logic.

use ropey::RopeSlice;
use xeno_primitives::{Direction, Range};

use super::make_range;

/// Moves the cursor to the next or previous diff change.
pub fn move_to_diff_change(text: RopeSlice, range: Range, direction: Direction, count: usize, extend: bool) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let mut pos = range.head;

	for _ in 0..count {
		match direction {
			Direction::Forward => {
				// Move to next line
				pos = next_line_start(text, pos);
				while pos < len && !is_diff_change(text, pos) {
					pos = next_line_start(text, pos);
				}
			}
			Direction::Backward => {
				// Move to previous line
				if pos > 0 {
					pos = prev_line_start(text, pos);
				}
				while pos > 0 && !is_diff_change(text, pos) {
					pos = prev_line_start(text, pos);
				}
			}
		}
	}

	make_range(range, pos, extend)
}

fn is_diff_change(text: RopeSlice, pos: usize) -> bool {
	let line_idx = text.char_to_line(pos);
	let line = text.line(line_idx);
	if line.len_chars() > 0 {
		let c = line.char(0);
		c == '+' || c == '-' || c == '@'
	} else {
		false
	}
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
