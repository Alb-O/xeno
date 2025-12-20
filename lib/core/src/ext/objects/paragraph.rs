//! Paragraph text object.

use ropey::RopeSlice;

use crate::range::Range;

fn is_blank_line(text: RopeSlice, line_idx: usize) -> bool {
	let line = text.line(line_idx);
	line.chars().all(|c| c.is_whitespace())
}

fn paragraph_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	if text.len_chars() == 0 {
		return None;
	}

	let line = text.char_to_line(pos);
	let total_lines = text.len_lines();

	// If we're on a blank line, select consecutive blank lines
	if is_blank_line(text, line) {
		let mut start_line = line;
		let mut end_line = line;

		while start_line > 0 && is_blank_line(text, start_line - 1) {
			start_line -= 1;
		}
		while end_line + 1 < total_lines && is_blank_line(text, end_line + 1) {
			end_line += 1;
		}

		let start = text.line_to_char(start_line);
		let end = if end_line + 1 < total_lines {
			text.line_to_char(end_line + 1)
		} else {
			text.len_chars()
		};

		return Some(Range::new(start, end));
	}

	// Find paragraph boundaries (blank lines)
	let mut start_line = line;
	while start_line > 0 && !is_blank_line(text, start_line - 1) {
		start_line -= 1;
	}

	let mut end_line = line;
	while end_line + 1 < total_lines && !is_blank_line(text, end_line + 1) {
		end_line += 1;
	}

	let start = text.line_to_char(start_line);
	let end = if end_line + 1 < total_lines {
		text.line_to_char(end_line + 1)
	} else {
		text.len_chars()
	};

	Some(Range::new(start, end))
}

fn paragraph_around(text: RopeSlice, pos: usize) -> Option<Range> {
	let inner = paragraph_inner(text, pos)?;
	let total_lines = text.len_lines();

	// "Around" includes trailing blank lines
	let end_line = text.char_to_line(inner.head.saturating_sub(1));
	let mut new_end_line = end_line;

	while new_end_line + 1 < total_lines && is_blank_line(text, new_end_line + 1) {
		new_end_line += 1;
	}

	let end = if new_end_line + 1 < total_lines {
		text.line_to_char(new_end_line + 1)
	} else {
		text.len_chars()
	};

	Some(Range::new(inner.anchor, end))
}

use crate::text_object;

text_object!(
	paragraph,
	{ trigger: 'p', description: "Select paragraph" },
	{
		inner: paragraph_inner,
		around: paragraph_around,
	}
);
