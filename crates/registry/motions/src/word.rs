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

/// Move to next word end (`e` command).
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

		let start_char = text.char(pos);
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
	}

	make_range_select(
		range,
		pos.saturating_sub(1).min(len.saturating_sub(1)),
		extend,
	)
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
}
