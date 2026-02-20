//! Word movement logic.

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Direction, Range};

use super::{WordBoundary, WordType, is_word_char, make_range_select};

pub fn move_word(text: RopeSlice, range: Range, direction: Direction, boundary: WordBoundary, count: usize, extend: bool) -> Range {
	match (direction, boundary) {
		(Direction::Forward, WordBoundary::Start) => move_to_next_word_start(text, range, count, WordType::Word, extend),
		(Direction::Forward, WordBoundary::End) => move_to_next_word_end(text, range, count, WordType::Word, extend),
		(Direction::Backward, WordBoundary::Start) => move_to_prev_word_start(text, range, count, WordType::Word, extend),
		_ => range, // Not implemented
	}
}

/// Move to next word start.
pub fn move_to_next_word_start(text: RopeSlice, range: Range, count: usize, word_type: WordType, extend: bool) -> Range {
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

		while pos < len && text.char(pos).is_whitespace() {
			let c = text.char(pos);
			if c == '\n' {
				pos += 1;
				break;
			}
			pos += 1;
		}
	}

	make_range_select(range, pos.min(len), extend)
}

/// Move to next word end.
pub fn move_to_next_word_end(text: RopeSlice, range: Range, count: usize, word_type: WordType, extend: bool) -> Range {
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

	let end_pos = pos.saturating_sub(1).min(len.saturating_sub(1));
	make_range_select(range, end_pos, extend)
}

/// Move to previous word start.
pub fn move_to_prev_word_start(text: RopeSlice, range: Range, count: usize, word_type: WordType, extend: bool) -> Range {
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
