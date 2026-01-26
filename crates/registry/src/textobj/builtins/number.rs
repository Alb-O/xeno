//! Number text object.

use ropey::RopeSlice;
use xeno_primitives::Range;

use crate::textobj::symmetric_text_object;

/// Returns whether a character is a digit or numeric separator.
fn is_digit_or_separator(ch: char) -> bool {
	ch.is_ascii_digit() || ch == '_' || ch == '.'
}

/// Returns whether a character can be part of a number literal.
fn is_number_char(ch: char, allow_prefix: bool) -> bool {
	ch.is_ascii_digit()
		|| ch == '_'
		|| ch == '.'
		|| ch == 'x'
		|| ch == 'X'
		|| ch == 'b'
		|| ch == 'B'
		|| ch == 'o'
		|| ch == 'O'
		|| (allow_prefix && (ch == '-' || ch == '+'))
		|| ('a'..='f').contains(&ch)
		|| ('A'..='F').contains(&ch)
		|| ch == 'e'
		|| ch == 'E'
}

/// Selects the number literal at the cursor position.
fn number_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	let len = text.len_chars();
	if len == 0 {
		return None;
	}

	let current = text.char(pos);
	if !current.is_ascii_digit() && current != '.' && current != '-' && current != '+' {
		return None;
	}

	let mut start = pos;
	let mut end = pos;

	while start > 0 {
		let ch = text.char(start - 1);
		if is_number_char(ch, start == pos) {
			start -= 1;
		} else {
			break;
		}
	}

	while end < len {
		let ch = text.char(end);
		if is_digit_or_separator(ch)
			|| ('a'..='f').contains(&ch)
			|| ('A'..='F').contains(&ch)
			|| ch == 'x'
			|| ch == 'X'
			|| ch == 'b'
			|| ch == 'B'
			|| ch == 'o'
			|| ch == 'O'
			|| ch == 'e'
			|| ch == 'E'
			|| ch == '+'
			|| ch == '-'
		{
			end += 1;
		} else {
			break;
		}
	}

	if start == end {
		return None;
	}

	Some(Range::new(start, end))
}

symmetric_text_object!(
	number,
	{ trigger: 'n', description: "Select number" },
	number_inner
);
