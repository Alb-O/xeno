use ropey::RopeSlice;
use unicode_segmentation::UnicodeSegmentation;

use crate::range::CharIdx;

/// Returns whether `char_idx` is at a grapheme cluster boundary.
///
/// Boundaries occur at the start/end of text and between grapheme clusters.
pub fn is_grapheme_boundary(text: RopeSlice, char_idx: CharIdx) -> bool {
	if char_idx == 0 || char_idx == text.len_chars() {
		return true;
	}

	let start: CharIdx = char_idx.saturating_sub(1);
	let end: CharIdx = (char_idx + 1).min(text.len_chars());
	let chunk: String = text.slice(start..end).into();

	let graphemes: Vec<&str> = chunk.graphemes(true).collect();
	graphemes.len() > 1
}

/// Returns the char index of the next grapheme cluster boundary after `char_idx`.
///
/// If `char_idx` is at or past the end, returns `text.len_chars()`.
pub fn next_grapheme_boundary(text: RopeSlice, char_idx: CharIdx) -> CharIdx {
	let len = text.len_chars();
	if char_idx >= len {
		return len;
	}

	let mut idx: CharIdx = char_idx + 1;
	while idx < len && !is_grapheme_boundary(text, idx) {
		idx += 1;
	}
	idx
}

/// Returns the char index of the previous grapheme cluster boundary before `char_idx`.
///
/// If `char_idx` is 0, returns 0.
pub fn prev_grapheme_boundary(text: RopeSlice, char_idx: CharIdx) -> CharIdx {
	if char_idx == 0 {
		return 0;
	}

	let mut idx: CharIdx = char_idx - 1;
	while idx > 0 && !is_grapheme_boundary(text, idx) {
		idx -= 1;
	}
	idx
}

/// Snaps `char_idx` to the next grapheme boundary if not already on one.
pub fn ensure_grapheme_boundary_next(text: RopeSlice, char_idx: CharIdx) -> CharIdx {
	if is_grapheme_boundary(text, char_idx) {
		char_idx
	} else {
		next_grapheme_boundary(text, char_idx)
	}
}

/// Snaps `char_idx` to the previous grapheme boundary if not already on one.
pub fn ensure_grapheme_boundary_prev(text: RopeSlice, char_idx: CharIdx) -> CharIdx {
	if is_grapheme_boundary(text, char_idx) {
		char_idx
	} else {
		prev_grapheme_boundary(text, char_idx)
	}
}

#[cfg(test)]
mod tests;
