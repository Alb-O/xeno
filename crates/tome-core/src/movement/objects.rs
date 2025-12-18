//! Text object selection (words, surrounds, etc).

use ropey::RopeSlice;

use super::{WordType, is_word_char};
use crate::range::Range;

/// Select a word object (inner or around).
/// Inner: just the word characters
/// Around: word + trailing whitespace (or leading if at end)
pub fn select_word_object(
	text: RopeSlice,
	range: Range,
	word_type: WordType,
	inner: bool,
) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let pos = range.head.min(len.saturating_sub(1));

	let is_word = match word_type {
		WordType::Word => is_word_char,
		WordType::WORD => |c: char| !c.is_whitespace(),
	};

	let c = text.char(pos);

	// If we're on whitespace, select the whitespace
	if !is_word(c) {
		let mut start = pos;
		let mut end = pos;

		// Extend backward through whitespace
		while start > 0 && !is_word(text.char(start - 1)) {
			start -= 1;
		}
		// Extend forward through whitespace
		while end + 1 < len && !is_word(text.char(end + 1)) {
			end += 1;
		}

		return Range::new(start, end);
	}

	let mut start = pos;
	let mut end = pos;

	while start > 0 && is_word(text.char(start - 1)) {
		start -= 1;
	}
	while end + 1 < len && is_word(text.char(end + 1)) {
		end += 1;
	}

	if inner {
		Range::new(start, end)
	} else {
		// Around: include trailing whitespace (or leading if at end of line/file)
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
			// No trailing space, try leading
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

/// Select a surround/paired object (parentheses, braces, quotes, etc).
/// Inner: content between delimiters (exclusive)
/// Around: content including delimiters (inclusive)
pub fn select_surround_object(
	text: RopeSlice,
	range: Range,
	open: char,
	close: char,
	inner: bool,
) -> Option<Range> {
	let len = text.len_chars();
	if len == 0 {
		return None;
	}

	let pos = range.head.min(len.saturating_sub(1));
	let balanced = open != close;

	// Find opening delimiter (search backward)
	let mut open_pos = None;
	let mut depth = 0i32;
	let mut search_pos = pos;

	// First check if we're on a delimiter
	let c = text.char(pos);
	if c == open {
		open_pos = Some(pos);
	} else if c == close && balanced {
		depth = 1;
	}

	if open_pos.is_none() {
		// Search backward for opening
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
			} else {
				// Quotes: just find the nearest one
				if c == open {
					open_pos = Some(search_pos);
					break;
				}
			}
		}
	}

	let open_pos = open_pos?;

	// Find closing delimiter (search forward from opening)
	let mut close_pos = None;
	let mut depth = 0i32;
	let mut search_pos = open_pos + 1;

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
		} else {
			// Quotes: just find the next one
			if c == close {
				close_pos = Some(search_pos);
				break;
			}
		}
		search_pos += 1;
	}

	let close_pos = close_pos?;

	if inner {
		// Inner: between delimiters (exclusive)
		if close_pos > open_pos + 1 {
			Some(Range::new(open_pos + 1, close_pos - 1))
		} else {
			// Empty content
			Some(Range::point(open_pos + 1))
		}
	} else {
		// Around: including delimiters
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

		// Cursor on 'e' in hello
		let range = Range::point(1);
		let selected = select_word_object(slice, range, WordType::Word, true);
		assert_eq!(selected.from(), 0);
		assert_eq!(selected.to(), 4); // "hello" is positions 0-4

		// Cursor on 'o' in world
		let range = Range::point(7);
		let selected = select_word_object(slice, range, WordType::Word, true);
		assert_eq!(selected.from(), 6);
		assert_eq!(selected.to(), 10); // "world" is positions 6-10
	}

	#[test]
	fn test_select_word_object_around() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);

		// Cursor on 'e' in hello - around includes trailing space
		let range = Range::point(1);
		let selected = select_word_object(slice, range, WordType::Word, false);
		assert_eq!(selected.from(), 0);
		assert_eq!(selected.to(), 5); // "hello " is positions 0-5
	}

	#[test]
	fn test_select_surround_object_parens() {
		let text = Rope::from("foo(bar)baz");
		let slice = text.slice(..);

		// Cursor inside parens on 'a'
		let range = Range::point(5);

		// Inner: just "bar"
		let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
		assert_eq!(selected.from(), 4);
		assert_eq!(selected.to(), 6); // "bar" is positions 4-6

		// Around: "(bar)"
		let selected = select_surround_object(slice, range, '(', ')', false).unwrap();
		assert_eq!(selected.from(), 3);
		assert_eq!(selected.to(), 7); // "(bar)" is positions 3-7
	}

	#[test]
	fn test_select_surround_object_nested() {
		let text = Rope::from("foo(a(b)c)bar");
		let slice = text.slice(..);

		// Cursor on 'b' inside inner parens
		let range = Range::point(6);
		let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
		assert_eq!(selected.from(), 6);
		assert_eq!(selected.to(), 6); // inner of (b) is just "b"

		// Cursor on 'a' - should get inner of outer parens
		let range = Range::point(4);
		let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
		assert_eq!(selected.from(), 4);
		assert_eq!(selected.to(), 8); // inner of (a(b)c) is "a(b)c"
	}

	#[test]
	fn test_select_surround_object_quotes() {
		let text = Rope::from(r#"say "hello" now"#);
		let slice = text.slice(..);

		// Cursor on 'e' inside quotes
		let range = Range::point(6);

		// Inner: just "hello"
		let selected = select_surround_object(slice, range, '"', '"', true).unwrap();
		assert_eq!(selected.from(), 5);
		assert_eq!(selected.to(), 9); // "hello" is positions 5-9

		// Around: "\"hello\""
		let selected = select_surround_object(slice, range, '"', '"', false).unwrap();
		assert_eq!(selected.from(), 4);
		assert_eq!(selected.to(), 10);
	}
}
