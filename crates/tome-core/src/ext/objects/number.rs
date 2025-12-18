//! Number text object.

use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{TEXT_OBJECTS, TextObjectDef};
use crate::range::Range;

fn is_digit_or_separator(ch: char) -> bool {
	ch.is_ascii_digit() || ch == '_' || ch == '.'
}

/// Check if a character is part of a number literal.
///
/// Supports various number formats:
/// - Decimal: `123`, `3.14`, `1_000_000`
/// - Hexadecimal: `0xFF`, `0xDEADBEEF`
/// - Binary: `0b1010`, `0B11110000`
/// - Octal: `0o755`, `0O644`
/// - Scientific notation: `1.23e10`, `4.56E-8`
/// - Signed numbers: `-42`, `+3.14`
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

fn number_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	let len = text.len_chars();
	if len == 0 {
		return None;
	}

	// Check if we're on/near a digit
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

fn number_around(text: RopeSlice, pos: usize) -> Option<Range> {
	// For numbers, "around" is the same as "inner" - no surrounding delimiters
	number_inner(text, pos)
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_NUMBER: TextObjectDef = TextObjectDef {
	name: "number",
	trigger: 'n',
	alt_triggers: &[],
	description: "Select number",
	inner: number_inner,
	around: number_around,
};
