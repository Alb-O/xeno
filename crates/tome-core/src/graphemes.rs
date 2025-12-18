use ropey::RopeSlice;
use unicode_segmentation::UnicodeSegmentation;

pub fn is_grapheme_boundary(text: RopeSlice, char_idx: usize) -> bool {
	if char_idx == 0 || char_idx == text.len_chars() {
		return true;
	}

	let start = char_idx.saturating_sub(1);
	let end = (char_idx + 1).min(text.len_chars());
	let chunk: String = text.slice(start..end).into();

	let graphemes: Vec<&str> = chunk.graphemes(true).collect();
	graphemes.len() > 1
}

pub fn next_grapheme_boundary(text: RopeSlice, char_idx: usize) -> usize {
	let len = text.len_chars();
	if char_idx >= len {
		return len;
	}

	let mut idx = char_idx + 1;
	while idx < len && !is_grapheme_boundary(text, idx) {
		idx += 1;
	}
	idx
}

pub fn prev_grapheme_boundary(text: RopeSlice, char_idx: usize) -> usize {
	if char_idx == 0 {
		return 0;
	}

	let mut idx = char_idx - 1;
	while idx > 0 && !is_grapheme_boundary(text, idx) {
		idx -= 1;
	}
	idx
}

pub fn ensure_grapheme_boundary_next(text: RopeSlice, char_idx: usize) -> usize {
	if is_grapheme_boundary(text, char_idx) {
		char_idx
	} else {
		next_grapheme_boundary(text, char_idx)
	}
}

pub fn ensure_grapheme_boundary_prev(text: RopeSlice, char_idx: usize) -> usize {
	if is_grapheme_boundary(text, char_idx) {
		char_idx
	} else {
		prev_grapheme_boundary(text, char_idx)
	}
}

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_grapheme_boundaries() {
		let text = Rope::from("hello");
		let slice = text.slice(..);

		assert!(is_grapheme_boundary(slice, 0));
		assert!(is_grapheme_boundary(slice, 5));
		assert_eq!(next_grapheme_boundary(slice, 0), 1);
		assert_eq!(prev_grapheme_boundary(slice, 5), 4);
	}

	#[test]
	fn test_emoji_graphemes() {
		let text = Rope::from("a😀b");
		let slice = text.slice(..);

		assert!(is_grapheme_boundary(slice, 0));
		assert!(is_grapheme_boundary(slice, 1));
		assert_eq!(next_grapheme_boundary(slice, 1), 2);
		assert_eq!(next_grapheme_boundary(slice, 2), 3);
	}
}
