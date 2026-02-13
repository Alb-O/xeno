//! Word movement functions (`w`, `b`, `e` commands).

use ropey::RopeSlice;
use xeno_primitives::range::{CharIdx, Range};

use super::{WordType, is_word_char, make_range_select};

/// Moves to the start of the next word (`w` command).
///
/// Advances past the current word boundary and any following whitespace,
/// stopping at a newline if encountered.
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
			if text.char(pos) == '\n' {
				pos += 1;
				break;
			}
			pos += 1;
		}
	}

	make_range_select(range, pos.min(len), extend)
}

/// Moves to the end of the next word (`e` command).
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

/// Moves to the start of the previous word (`b` command).
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

#[cfg(test)]
mod tests;
