//! Text object selection (words, surrounds, etc).

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Range};

use super::{WordType, is_word_char};

/// Selects a word text object.
///
/// `inner` selects only the word characters. `around` extends to include
/// trailing whitespace, or leading whitespace if no trailing space exists.
pub fn select_word_object(text: RopeSlice, range: Range, word_type: WordType, inner: bool) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let pos: CharIdx = range.head.min(len.saturating_sub(1));

	let is_word = match word_type {
		WordType::Word => is_word_char,
		WordType::WORD => |c: char| !c.is_whitespace(),
	};

	let c = text.char(pos);

	if !is_word(c) {
		let mut start: CharIdx = pos;
		let mut end: CharIdx = pos;

		while start > 0 && !is_word(text.char(start - 1)) {
			start -= 1;
		}
		while end + 1 < len && !is_word(text.char(end + 1)) {
			end += 1;
		}

		return Range::new(start, end);
	}

	let mut start: CharIdx = pos;
	let mut end: CharIdx = pos;

	while start > 0 && is_word(text.char(start - 1)) {
		start -= 1;
	}
	while end + 1 < len && is_word(text.char(end + 1)) {
		end += 1;
	}

	if inner {
		Range::new(start, end)
	} else {
		let mut around_end = end;
		while around_end + 1 < len {
			let next_c = text.char(around_end + 1);
			if next_c.is_whitespace() && next_c != '\n' {
				around_end += 1;
			} else {
				break;
			}
		}

		if around_end > end {
			Range::new(start, around_end)
		} else {
			let mut around_start = start;
			while around_start > 0 {
				let prev_c = text.char(around_start - 1);
				if prev_c.is_whitespace() && prev_c != '\n' {
					around_start -= 1;
				} else {
					break;
				}
			}
			Range::new(around_start, end)
		}
	}
}

/// Selects a surround/paired text object (parentheses, braces, quotes, etc).
///
/// `inner` selects the content between delimiters (exclusive).
/// `around` includes the delimiters themselves (inclusive).
/// Returns `None` if no matching delimiter pair is found around the cursor.
pub fn select_surround_object(text: RopeSlice, range: Range, open: char, close: char, inner: bool) -> Option<Range> {
	let len = text.len_chars();
	if len == 0 {
		return None;
	}

	let pos: CharIdx = range.head.min(len.saturating_sub(1));
	let balanced = open != close;

	let mut open_pos: Option<CharIdx> = None;
	let mut depth = 0i32;
	let mut search_pos: CharIdx = pos;

	let c = text.char(pos);
	if c == open {
		open_pos = Some(pos);
	} else if c == close && balanced {
		depth = 1;
	}

	if open_pos.is_none() {
		while search_pos > 0 {
			search_pos -= 1;
			let c = text.char(search_pos);
			if balanced {
				if c == close {
					depth += 1;
				} else if c == open {
					if depth == 0 {
						open_pos = Some(search_pos);
						break;
					}
					depth -= 1;
				}
			} else if c == open {
				open_pos = Some(search_pos);
				break;
			}
		}
	}

	let open_pos = open_pos?;

	let mut close_pos: Option<CharIdx> = None;
	let mut depth = 0i32;
	let mut search_pos: CharIdx = open_pos + 1;

	while search_pos < len {
		let c = text.char(search_pos);
		if balanced {
			if c == open {
				depth += 1;
			} else if c == close {
				if depth == 0 {
					close_pos = Some(search_pos);
					break;
				}
				depth -= 1;
			}
		} else if c == close {
			close_pos = Some(search_pos);
			break;
		}
		search_pos += 1;
	}

	let close_pos = close_pos?;

	if inner {
		if close_pos > open_pos + 1 {
			Some(Range::new(open_pos + 1, close_pos - 1))
		} else {
			Some(Range::point(open_pos + 1))
		}
	} else {
		Some(Range::new(open_pos, close_pos))
	}
}

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_select_word_object_inner() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);

		let range = Range::point(1);
		let selected = select_word_object(slice, range, WordType::Word, true);
		assert_eq!(selected.min(), 0);
		assert_eq!(selected.max(), 4);

		let range = Range::point(7);
		let selected = select_word_object(slice, range, WordType::Word, true);
		assert_eq!(selected.min(), 6);
		assert_eq!(selected.max(), 10);
	}

	#[test]
	fn test_select_word_object_around() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);

		let range = Range::point(1);
		let selected = select_word_object(slice, range, WordType::Word, false);
		assert_eq!(selected.min(), 0);
		assert_eq!(selected.max(), 5);
	}

	#[test]
	fn test_select_surround_object_parens() {
		let text = Rope::from("foo(bar)baz");
		let slice = text.slice(..);

		let range = Range::point(5);

		let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
		assert_eq!(selected.min(), 4);
		assert_eq!(selected.max(), 6);

		let selected = select_surround_object(slice, range, '(', ')', false).unwrap();
		assert_eq!(selected.min(), 3);
		assert_eq!(selected.max(), 7);
	}

	#[test]
	fn test_select_surround_object_nested() {
		let text = Rope::from("foo(a(b)c)bar");
		let slice = text.slice(..);

		let range = Range::point(6);
		let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
		assert_eq!(selected.min(), 6);
		assert_eq!(selected.max(), 6);

		let range = Range::point(4);
		let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
		assert_eq!(selected.min(), 4);
		assert_eq!(selected.max(), 8);
	}

	#[test]
	fn test_select_surround_object_quotes() {
		let text = Rope::from(r#"say "hello" now"#);
		let slice = text.slice(..);

		let range = Range::point(6);

		let selected = select_surround_object(slice, range, '"', '"', true).unwrap();
		assert_eq!(selected.min(), 5);
		assert_eq!(selected.max(), 9);

		let selected = select_surround_object(slice, range, '"', '"', false).unwrap();
		assert_eq!(selected.min(), 4);
		assert_eq!(selected.max(), 10);
	}
}
