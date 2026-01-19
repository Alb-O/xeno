//! Diff hunk navigation motions.

use ropey::RopeSlice;
use xeno_primitives::range::Range;

use crate::movement::make_range;

/// Returns true if the line at `line_idx` is a hunk header (starts with `@@`).
fn is_hunk_header(text: RopeSlice, line_idx: usize) -> bool {
	let line = text.line(line_idx);
	let mut chars = line.chars();
	chars.next() == Some('@') && chars.next() == Some('@')
}

/// Moves to the next diff hunk header.
///
/// A diff hunk header starts with `@@`. This function skips forward to the
/// next line that starts with `@@`.
pub fn move_to_next_hunk(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	let total_lines = text.len_lines();
	if total_lines == 0 {
		return range;
	}

	let mut line = text.char_to_line(range.head);
	for _ in 0..count {
		if line >= total_lines.saturating_sub(1) {
			break;
		}
		line += 1;
		while line < total_lines.saturating_sub(1) && !is_hunk_header(text, line) {
			line += 1;
		}
		if !is_hunk_header(text, line) && line == total_lines.saturating_sub(1) {
			break;
		}
	}

	make_range(range, text.line_to_char(line), extend)
}

/// Moves to the previous diff hunk header.
///
/// A diff hunk header starts with `@@`. This function moves backwards to the
/// previous line that starts with `@@`.
pub fn move_to_prev_hunk(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range {
	let total_lines = text.len_lines();
	if total_lines == 0 {
		return range;
	}

	let mut line = text.char_to_line(range.head);
	for _ in 0..count {
		if line == 0 {
			break;
		}
		line -= 1;
		while line > 0 && !is_hunk_header(text, line) {
			line -= 1;
		}
	}

	make_range(range, text.line_to_char(line), extend)
}

motion!(
	next_hunk,
	{ description: "Move to next diff hunk" },
	|text, range, count, extend| move_to_next_hunk(text, range, count, extend)
);

motion!(
	prev_hunk,
	{ description: "Move to previous diff hunk" },
	|text, range, count, extend| move_to_prev_hunk(text, range, count, extend)
);

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	const DIFF_TEXT: &str = "\
diff --git a/file.rs b/file.rs
index 1234567..abcdefg 100644
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!(\"hello\");
 }
@@ -10,5 +11,6 @@
 fn other() {
+    println!(\"world\");
 }
@@ -20,3 +22,4 @@
 fn last() {
+    println!(\"!\");
 }";

	#[test]
	fn test_next_hunk() {
		let text = Rope::from(DIFF_TEXT);
		let slice = text.slice(..);

		// Start at line 0, should jump to first @@ at line 4
		let moved = move_to_next_hunk(slice, Range::point(0), 1, false);
		assert_eq!(text.char_to_line(moved.head), 4);

		// From first @@, should jump to second @@ at line 8
		let moved = move_to_next_hunk(slice, moved, 1, false);
		assert_eq!(text.char_to_line(moved.head), 8);

		// From second @@, should jump to third @@ at line 12
		let moved = move_to_next_hunk(slice, moved, 1, false);
		assert_eq!(text.char_to_line(moved.head), 12);
	}

	#[test]
	fn test_prev_hunk() {
		let text = Rope::from(DIFF_TEXT);
		let slice = text.slice(..);

		// Start at the last @@ (line 12), go back to line 8
		let start_char = text.line_to_char(12);
		let moved = move_to_prev_hunk(slice, Range::point(start_char), 1, false);
		assert_eq!(text.char_to_line(moved.head), 8);

		// From line 8, go back to line 4
		let moved = move_to_prev_hunk(slice, moved, 1, false);
		assert_eq!(text.char_to_line(moved.head), 4);
	}

	#[test]
	fn test_hunk_with_count() {
		let text = Rope::from(DIFF_TEXT);
		let slice = text.slice(..);

		// Jump 2 hunks forward from start
		let moved = move_to_next_hunk(slice, Range::point(0), 2, false);
		assert_eq!(text.char_to_line(moved.head), 8);

		// Jump 3 hunks forward from start
		let moved = move_to_next_hunk(slice, Range::point(0), 3, false);
		assert_eq!(text.char_to_line(moved.head), 12);
	}

	#[test]
	fn test_no_hunks() {
		let text = Rope::from("just some\nplain text\nwithout hunks");
		let slice = text.slice(..);

		// Should stay at end of file
		let moved = move_to_next_hunk(slice, Range::point(0), 1, false);
		assert_eq!(text.char_to_line(moved.head), 2);
	}
}
