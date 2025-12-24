//! Line text object.

use ropey::RopeSlice;

use crate::range::Range;

fn line_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	if text.len_chars() == 0 {
		return None;
	}
	let line = text.char_to_line(pos);
	let start = text.line_to_char(line);
	let line_len = text.line(line).len_chars();

	let end = if line_len > 0 {
		let line_text = text.line(line);
		let last_char = line_text.char(line_len - 1);
		if last_char == '\n' {
			start + line_len - 1
		} else {
			start + line_len
		}
	} else {
		start
	};

	Some(Range::new(start, end))
}

fn line_around(text: RopeSlice, pos: usize) -> Option<Range> {
	if text.len_chars() == 0 {
		return None;
	}
	let line = text.char_to_line(pos);
	let start = text.line_to_char(line);
	let end = if line + 1 < text.len_lines() {
		text.line_to_char(line + 1)
	} else {
		text.len_chars()
	};

	Some(Range::new(start, end))
}

use crate::text_object;

text_object!(
	line,
	{ trigger: 'x', description: "Select line" },
	{
		inner: line_inner,
		around: line_around,
	}
);
