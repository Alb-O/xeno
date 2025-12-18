//! Character find functions (Kakoune's f, t, F, T commands).

use ropey::RopeSlice;

use super::make_range_select;
use crate::range::Range;

/// Find character forward (Kakoune's `f` and `t` commands).
///
/// # Arguments
/// * `inclusive` - If true, includes the target character (`f` command).
///   If false, stops before the target (`t` command).
/// * `count` - Number of occurrences to skip (e.g., `2f` finds second 'f').
///
/// # Examples
/// ```ignore
/// // Text: "hello world"
/// // Position: 0 (at 'h')
///
/// // f command (inclusive): finds 'o', moves to position 4
/// find_char_forward(text, range, 'o', 1, true, false);
///
/// // t command (exclusive): finds 'o', moves to position 3 (before 'o')
/// find_char_forward(text, range, 'o', 1, false, false);
///
/// // 2f command: finds second 'o', moves to position 7
/// find_char_forward(text, range, 'o', 2, true, false);
/// ```
pub fn find_char_forward(
	text: RopeSlice,
	range: Range,
	target: char,
	count: usize,
	inclusive: bool,
	extend: bool,
) -> Range {
	let len = text.len_chars();
	let mut pos = range.head + 1;
	let mut found_count = 0;

	while pos < len {
		if text.char(pos) == target {
			found_count += 1;
			if found_count >= count {
				let final_pos = if inclusive {
					pos
				} else {
					pos.saturating_sub(1)
				};
				return make_range_select(range, final_pos, extend);
			}
		}
		pos += 1;
	}

	range
}

/// Find character backward (Kakoune's `alt-f` and `alt-t` commands).
pub fn find_char_backward(
	text: RopeSlice,
	range: Range,
	target: char,
	count: usize,
	inclusive: bool,
	extend: bool,
) -> Range {
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

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_find_char_forward() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = find_char_forward(slice, range, 'o', 1, true, false);
		assert_eq!(moved.head, 4);

		let moved = find_char_forward(slice, range, 'o', 1, false, false);
		assert_eq!(moved.head, 3);
	}

	#[test]
	fn test_find_char_forward_count() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(0);

		let moved = find_char_forward(slice, range, 'o', 2, true, false);
		assert_eq!(moved.head, 7);
	}

	#[test]
	fn test_find_char_backward() {
		let text = Rope::from("hello world");
		let slice = text.slice(..);
		let range = Range::point(10);

		let moved = find_char_backward(slice, range, 'o', 1, true, false);
		assert_eq!(moved.head, 7);
	}
}
