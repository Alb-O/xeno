//! Word-based cursor movement (w, b, e, W, B, E commands).

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Range};

use crate::movement::{WordType, is_word_char, make_range_select};

/// Move to next word start (`w` command).
/// Selects the word and following whitespace on the right.
pub fn move_to_next_word_start(
	text: RopeSlice,
	range: Range,
	count: usize,
	word_type: WordType,
	extend: bool,
) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let mut pos: CharIdx = range.head;

	for _ in 0..count {
		if pos >= len {
			break;
		}

		let start_char = text.char(pos.min(len.saturating_sub(1)));
		let start_is_word = match word_type {
			WordType::Word => is_word_char(start_char),
			WordType::WORD => !start_char.is_whitespace(),
		};

		while pos < len {
			let c = text.char(pos);
			let is_word = match word_type {
				WordType::Word => is_word_char(c),
				WordType::WORD => !c.is_whitespace(),
			};
			if is_word != start_is_word {
				break;
			}
			pos += 1;
		}

		while pos < len {
			let c = text.char(pos);
			if !c.is_whitespace() {
				break;
			}
			pos += 1;
			if c == '\n' {
				break;
			}
		}
	}

	make_range_select(range, pos.min(len), extend)
}

/// Moves to next word end.
///
/// In [`WordType::Word`] mode, skips trailing punctuation at EOL rather than
/// targeting it. Selection anchor starts at the target word, not trailing
/// across the newline.
pub fn move_to_next_word_end(
	text: RopeSlice,
	range: Range,
	count: usize,
	word_type: WordType,
	extend: bool,
) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let mut pos: CharIdx = range.head;
	let mut word_start: Option<CharIdx> = None;
	let is_word_mode = matches!(word_type, WordType::Word);

	for _ in 0..count {
		if pos < len {
			pos += 1;
		}

		while pos < len && text.char(pos).is_whitespace() {
			pos += 1;
		}

		if pos >= len {
			break;
		}

		let on_word_char = is_word_mode && is_word_char(text.char(pos));

		// Skip EOL punctuation when no word chars remain on line
		if is_word_mode && !on_word_char {
			let line_has_word = (pos..len)
				.take_while(|&p| text.char(p) != '\n')
				.any(|p| is_word_char(text.char(p)));

			if !line_has_word {
				while pos < len && !is_word_char(text.char(pos)) {
					pos += 1;
				}
				if pos >= len {
					break;
				}
				word_start = Some(pos);
			}
		}

		let current_is_word = if is_word_mode {
			is_word_char(text.char(pos))
		} else {
			true // WORD mode: after skipping whitespace, always on a "word"
		};

		while pos < len {
			let c = text.char(pos);
			let is_word = if is_word_mode {
				is_word_char(c)
			} else {
				!c.is_whitespace()
			};
			if is_word != current_is_word {
				break;
			}
			pos += 1;
		}
	}

	let head = pos.saturating_sub(1).min(len.saturating_sub(1));
	let anchor = if extend {
		range.anchor
	} else {
		word_start.unwrap_or(range.head)
	};
	Range::new(anchor, head)
}

/// Move to previous word start (`b` command).
pub fn move_to_prev_word_start(
	text: RopeSlice,
	range: Range,
	count: usize,
	word_type: WordType,
	extend: bool,
) -> Range {
	let len = text.len_chars();
	if len == 0 {
		return range;
	}

	let mut pos: CharIdx = range.head;

	for _ in 0..count {
		pos = pos.saturating_sub(1);

		while pos > 0 && text.char(pos).is_whitespace() {
			pos -= 1;
		}

		if pos == 0 {
			break;
		}

		let start_char = text.char(pos);
		let start_is_word = match word_type {
			WordType::Word => is_word_char(start_char),
			WordType::WORD => !start_char.is_whitespace(),
		};

		while pos > 0 {
			let prev_char = text.char(pos - 1);
			let is_word = match word_type {
				WordType::Word => is_word_char(prev_char),
				WordType::WORD => !prev_char.is_whitespace(),
			};
			if is_word != start_is_word {
				break;
			}
			pos -= 1;
		}
	}

	make_range_select(range, pos, extend)
}

motion!(
	next_word_start,
	{ description: "Move to next word start" },
	|text, range, count, extend| {
		move_to_next_word_start(text, range, count, WordType::Word, extend)
	}
);

motion!(
	prev_word_start,
	{ description: "Move to previous word start" },
	|text, range, count, extend| {
		move_to_prev_word_start(text, range, count, WordType::Word, extend)
	}
);

motion!(
	next_word_end,
	{ description: "Move to next word end" },
	|text, range, count, extend| {
		move_to_next_word_end(text, range, count, WordType::Word, extend)
	}
);

motion!(
	next_WORD_start,
	{ description: "Move to next WORD start" },
	|text, range, count, extend| {
		move_to_next_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	next_long_word_start,
	{ description: "Move to next WORD start" },
	|text, range, count, extend| {
		move_to_next_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	prev_WORD_start,
	{ description: "Move to previous WORD start" },
	|text, range, count, extend| {
		move_to_prev_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	prev_long_word_start,
	{ description: "Move to previous WORD start" },
	|text, range, count, extend| {
		move_to_prev_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	next_WORD_end,
	{ description: "Move to next WORD end" },
	|text, range, count, extend| {
		move_to_next_word_end(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	next_long_word_end,
	{ description: "Move to next WORD end" },
	|text, range, count, extend| {
		move_to_next_word_end(text, range, count, WordType::WORD, extend)
	}
);

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_move_to_next_word_start() {
		let text = Rope::from("hello world test");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_to_next_word_start(slice, range, 1, WordType::Word, false);
		assert_eq!(moved.head, 6);

		let moved2 = move_to_next_word_start(slice, moved, 1, WordType::Word, false);
		assert_eq!(moved2.head, 12);
	}

	#[test]
	fn test_move_to_next_word_start_count() {
		let text = Rope::from("one two three four");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_to_next_word_start(slice, range, 2, WordType::Word, false);
		assert_eq!(moved.head, 8);
	}

	#[test]
	fn test_move_to_prev_word_start() {
		let text = Rope::from("hello world test");
		let slice = text.slice(..);
		let range = Range::point(12);

		let moved = move_to_prev_word_start(slice, range, 1, WordType::Word, false);
		assert_eq!(moved.head, 6);
	}

	#[test]
	fn test_move_to_next_word_end() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_to_next_word_end(slice, range, 1, WordType::Word, false);
		assert_eq!(moved.head, 4);
	}

	#[test]
	fn test_word_movement_extend() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = move_to_next_word_start(slice, range, 1, WordType::Word, true);
		assert_eq!(moved.anchor, 0);
		assert_eq!(moved.head, 6);
	}

	#[test]
	fn test_word_end_skips_eol_punctuation() {
		// [profile.dev]\nnext_word
		// 0123456789...
		// When on 'v' (pos 11), pressing e should skip ']' and go to 'd' in next_word
		let text = Rope::from("[profile.dev]\nnext_word");
		let slice = text.slice(..);

		// Start on 'v' at position 11
		let range = Range::point(11);
		let moved = move_to_next_word_end(slice, range, 1, WordType::Word, false);

		// Should land on 'd' of next_word (position 22), not on ']'
		assert_eq!(moved.head, 22, "should skip EOL punctuation to next word");
		// Selection should start at 'n' of next_word (position 14), not trail back
		assert_eq!(moved.anchor, 14, "selection should start at word begin");
	}

	#[test]
	fn test_word_end_targets_mid_line_punctuation() {
		// foo.bar should still work normally - punctuation with words after it
		let text = Rope::from("foo.bar");
		let slice = text.slice(..);

		// Start on 'o' at position 2
		let range = Range::point(2);
		let moved = move_to_next_word_end(slice, range, 1, WordType::Word, false);

		// Should land on '.' (position 3) since there are word chars after it on the line
		assert_eq!(moved.head, 3, "should target punctuation when words follow");
	}

	#[test]
	fn test_word_end_word_mode() {
		// WORD mode (shift+E) should treat all non-whitespace as a word
		let text = Rope::from("foo.bar baz");
		let slice = text.slice(..);

		let range = Range::point(0);
		let moved = move_to_next_word_end(slice, range, 1, WordType::WORD, false);

		// Should land on 'r' (position 6) - end of "foo.bar" as a single WORD
		assert_eq!(moved.head, 6, "WORD mode should span punctuation");
	}
}
