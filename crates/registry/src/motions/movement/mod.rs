//! Movement functions for cursor and selection manipulation.
//!
//! This module provides shared utilities and re-exports movement functions
//! from their co-located modules.

mod find;
mod objects;
mod search;

pub use find::{find_char_backward, find_char_forward};
pub use objects::{select_surround_object, select_word_object};
pub use search::{escape_pattern, find_all_matches, find_next, find_prev, matches_pattern};
use xeno_primitives::range::{CharIdx, Range};

pub use crate::motions::builtins::document::{move_to_document_end, move_to_document_start};
pub use crate::motions::builtins::horizontal::move_horizontally;
pub use crate::motions::builtins::line::{
	move_to_first_nonwhitespace, move_to_line_end, move_to_line_start,
};
pub use crate::motions::builtins::paragraph::{move_to_next_paragraph, move_to_prev_paragraph};
pub use crate::motions::builtins::vertical::move_vertically;
pub use crate::motions::builtins::word::{
	move_to_next_word_end, move_to_next_word_start, move_to_prev_word_start,
};

/// Word type for word movements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordType {
	/// A word is alphanumeric characters (and those in extra_word_chars).
	Word,
	/// A WORD is any non-whitespace characters.
	WORD,
}

/// Returns whether a character is a word character (alphanumeric or underscore).
pub fn is_word_char(c: char) -> bool {
	c.is_alphanumeric() || c == '_'
}

/// Make a range for cursor movement - anchor stays, only head moves.
///
/// If `extend` is false, this performs a "move": the range collapses to a single point at `new_head`.
/// If `extend` is true, this performs a "selection extension": the anchor remains fixed, and the head moves to `new_head`.
pub fn make_range(range: Range, new_head: CharIdx, extend: bool) -> Range {
	if extend {
		Range::new(range.anchor, new_head)
	} else {
		Range::point(new_head)
	}
}

/// Creates a range for selection-creating motions.
///
/// With `extend`, keeps existing anchor. Without `extend`, anchor moves to old head position,
/// creating a new selection spanning from the previous cursor to the new position.
pub fn make_range_select(range: Range, new_head: CharIdx, extend: bool) -> Range {
	if extend {
		Range::new(range.anchor, new_head)
	} else {
		Range::new(range.head, new_head)
	}
}
