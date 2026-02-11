//! Movement functions for cursor and selection manipulation.
//!
//! This module provides shared utilities and re-exports movement functions
//! from their co-located modules.

mod diff;
mod document;
mod find;
mod horizontal;
mod line;
mod objects;
mod paragraph;
mod search;
mod vertical;
mod word;

pub use diff::*;
pub use document::*;
pub use find::*;
pub use horizontal::*;
pub use line::*;
pub use objects::*;
pub use paragraph::*;
pub use search::*;
pub use vertical::*;
pub use word::*;
use xeno_primitives::range::{CharIdx, Range};

/// Word boundary type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordBoundary {
	Start,
	End,
}

/// Line boundary type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBoundary {
	Start,
	End,
	FirstNonBlank,
}

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
	if extend { Range::new(range.anchor, new_head) } else { Range::point(new_head) }
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
