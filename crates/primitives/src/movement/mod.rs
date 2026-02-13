//! Cursor and selection movement primitives.

use ropey::RopeSlice;

use crate::range::{CharIdx, Range};

/// Returns whether a character is a word character (alphanumeric or underscore).
#[inline]
pub fn is_word_char(c: char) -> bool {
	c.is_alphanumeric() || c == '_'
}

/// Creates a range for cursor movement.
///
/// If `extend` is false, collapses to a point at `new_head`.
/// If `extend` is true, keeps anchor fixed, moves head to `new_head`.
#[inline]
pub fn make_range(range: Range, new_head: CharIdx, extend: bool) -> Range {
	if extend { Range::new(range.anchor, new_head) } else { Range::point(new_head) }
}

/// Creates a range for selection-extending motions.
///
/// With `extend`, keeps existing anchor. Otherwise, anchor moves to old head,
/// creating a selection from previous cursor to new position.
#[inline]
pub fn make_range_select(range: Range, new_head: CharIdx, extend: bool) -> Range {
	if extend {
		Range::new(range.anchor, new_head)
	} else {
		Range::new(range.head, new_head)
	}
}

/// Finds character forward (`f`/`t` commands).
///
/// With `inclusive`, lands on target (`f`). Otherwise stops before (`t`).
/// Skips `count - 1` occurrences for repeat motions.
pub fn find_char_forward(text: RopeSlice, range: Range, target: char, count: usize, inclusive: bool, extend: bool) -> Range {
	let len = text.len_chars();
	let mut pos = range.head + 1;
	let mut found_count = 0;

	while pos < len {
		if text.char(pos) == target {
			found_count += 1;
			if found_count >= count {
				let final_pos = if inclusive { pos } else { pos.saturating_sub(1) };
				return make_range_select(range, final_pos, extend);
			}
		}
		pos += 1;
	}

	range
}

/// Finds character backward (`F`/`T` commands).
pub fn find_char_backward(text: RopeSlice, range: Range, target: char, count: usize, inclusive: bool, extend: bool) -> Range {
	if range.head == 0 {
		return range;
	}

	let mut pos = range.head - 1;
	let mut found_count = 0;

	loop {
		if text.char(pos) == target {
			found_count += 1;
			if found_count >= count {
				let final_pos = if inclusive { pos } else { pos + 1 };
				return make_range_select(range, final_pos, extend);
			}
		}
		if pos == 0 {
			break;
		}
		pos -= 1;
	}

	range
}

/// Finds the start of the word containing or preceding `pos`.
pub fn find_word_start(text: RopeSlice, pos: CharIdx) -> CharIdx {
	let mut start = pos;
	while start > 0 && text.get_char(start - 1).is_some_and(is_word_char) {
		start -= 1;
	}
	start
}

/// Finds the end of the word containing or following `pos`.
pub fn find_word_end(text: RopeSlice, pos: CharIdx) -> CharIdx {
	let len = text.len_chars();
	let mut end = pos;
	while end + 1 < len && text.get_char(end + 1).is_some_and(is_word_char) {
		end += 1;
	}
	end
}

#[cfg(test)]
mod tests;
