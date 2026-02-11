//! Movement functions for cursor and selection manipulation.

mod find;
mod objects;
mod search;
mod word;

pub use find::{find_char_backward, find_char_forward};
pub use objects::{select_surround_object, select_word_object};
use ropey::RopeSlice;
pub use search::{escape_pattern, find_all_matches, find_next, find_next_re, find_prev, find_prev_re, matches_pattern};
pub use word::{move_to_next_word_end, move_to_next_word_start, move_to_prev_word_start};
use xeno_primitives::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use xeno_primitives::range::{CharIdx, Direction, Range};
use xeno_primitives::{max_cursor_pos, visible_line_count};

/// Vim-style word boundary classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordType {
	/// Alphanumeric + underscore boundaries.
	Word,
	/// Non-whitespace boundaries.
	WORD,
}

/// Word character predicate: alphanumeric or underscore.
pub(crate) fn is_word_char(c: char) -> bool {
	c.is_alphanumeric() || c == '_'
}

/// Produces a range for a cursor movement.
///
/// When `extend` is false, collapses to a point at `new_head`.
/// When `extend` is true, keeps the existing anchor.
pub(crate) fn make_range(range: Range, new_head: CharIdx, extend: bool) -> Range {
	if extend { Range::new(range.anchor, new_head) } else { Range::point(new_head) }
}

/// Produces a range for a selection-creating motion (e.g., `f`, `w`).
///
/// When `extend` is false, anchors at the old head position (creating a new
/// selection spanning old-head → new-head). When `extend` is true, keeps
/// the existing anchor.
pub(crate) fn make_range_select(range: Range, new_head: CharIdx, extend: bool) -> Range {
	if extend {
		Range::new(range.anchor, new_head)
	} else {
		Range::new(range.head, new_head)
	}
}

/// Moves the cursor horizontally by the given number of graphemes.
pub fn move_horizontally(text: RopeSlice, range: Range, direction: Direction, count: usize, extend: bool) -> Range {
	let pos: CharIdx = range.head;
	let max_pos = max_cursor_pos(text);
	let new_pos: CharIdx = match direction {
		Direction::Forward => {
			let mut p = pos;
			for _ in 0..count {
				let next = next_grapheme_boundary(text, p);
				if next > max_pos {
					break;
				}
				p = next;
			}
			p
		}
		Direction::Backward => {
			let mut p = pos;
			for _ in 0..count {
				p = prev_grapheme_boundary(text, p);
			}
			p
		}
	};

	make_range(range, new_pos, extend)
}

/// Moves the cursor vertically by the given number of lines.
pub fn move_vertically(text: RopeSlice, range: Range, direction: Direction, count: usize, extend: bool) -> Range {
	let pos: CharIdx = range.head;
	let line = text.char_to_line(pos);
	let line_start = text.line_to_char(line);
	let col = pos - line_start;

	let total_lines = visible_line_count(text);
	let new_line = match direction {
		Direction::Forward => (line + count).min(total_lines.saturating_sub(1)),
		Direction::Backward => line.saturating_sub(count),
	};

	let new_line_start = text.line_to_char(new_line);
	let new_line_content = text.line(new_line);
	let new_line_len = new_line_content.len_chars();
	let has_newline = new_line_len > 0 && new_line_content.char(new_line_len - 1) == '\n';
	let line_end_offset = if has_newline { new_line_len - 1 } else { new_line_len };

	let new_col = col.min(line_end_offset);
	let new_pos: CharIdx = new_line_start + new_col;

	make_range(range, new_pos, extend)
}

/// Moves the cursor to the start of the current line.
pub fn move_to_line_start(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start: CharIdx = text.line_to_char(line);
	make_range(range, line_start, extend)
}

/// Moves the cursor to the end of the current line.
///
/// Positions on the newline character if present, or at EOF for the final line
/// without a trailing newline.
pub fn move_to_line_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	let line_content = text.line(line);
	let line_len = line_content.len_chars();
	let has_newline = line_len > 0 && line_content.char(line_len - 1) == '\n';
	let line_end = line_start + if has_newline { line_len - 1 } else { line_len };
	make_range(range, line_end, extend)
}

/// Moves the cursor to the first non-whitespace character on the current line.
pub fn move_to_first_nonwhitespace(text: RopeSlice, range: Range, extend: bool) -> Range {
	let line = text.char_to_line(range.head);
	let line_start = text.line_to_char(line);
	let line_text = text.line(line);

	let mut first_non_ws: CharIdx = line_start;
	for (i, ch) in line_text.chars().enumerate() {
		if !ch.is_whitespace() {
			first_non_ws = line_start + i;
			break;
		}
	}

	make_range(range, first_non_ws, extend)
}

/// Moves the cursor to position 0.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, 0 as CharIdx, extend)
}

/// Moves the cursor to the last valid cursor position.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
	make_range(range, max_cursor_pos(text), extend)
}

#[cfg(test)]
mod tests;
