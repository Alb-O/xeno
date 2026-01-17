//! LSP change coalescing to reduce payload size.
//!
//! This module provides [`coalesce_changes`] which merges adjacent changes
//! to reduce the number of LSP notifications and total payload size.
//!
//! # Coalescing Rules
//!
//! Changes are coalesced when they are "adjacent" in the post-change coordinate
//! system. The following patterns are merged:
//!
//! 1. **Consecutive inserts**: Two inserts where the second starts at the end
//!    of the first's inserted text are merged into a single insert.
//!
//! 2. **Delete + Insert (replacement)**: A deletion followed by an insertion
//!    at the same position becomes a single replacement.
//!
//! 3. **Consecutive deletes**: Two deletions where the second starts at the
//!    same position as the first (after the first's deletion) are merged.
//!
//! # Coordinate System
//!
//! LSP changes are applied sequentially, with each change's range relative to
//! the document state *after* all previous changes. Coalescing preserves this
//! invariant by only merging changes that are logically adjacent.

use xeno_primitives::lsp::{LspDocumentChange, LspPosition, LspRange};

/// Coalesces a sequence of LSP changes to reduce payload size.
///
/// Returns a new vector with merged changes where possible. The coalesced
/// changes produce the same result when applied sequentially.
pub fn coalesce_changes(changes: Vec<LspDocumentChange>) -> Vec<LspDocumentChange> {
	if changes.len() < 2 {
		return changes;
	}

	let mut result: Vec<LspDocumentChange> = Vec::with_capacity(changes.len());

	for change in changes {
		if let Some(merged) = try_merge(result.last_mut(), &change) {
			*result.last_mut().unwrap() = merged;
		} else {
			result.push(change);
		}
	}

	result
}

/// Attempts to merge two consecutive changes.
///
/// Returns `Some(merged)` if the changes can be combined, `None` otherwise.
fn try_merge(prev: Option<&mut LspDocumentChange>, curr: &LspDocumentChange) -> Option<LspDocumentChange> {
	let prev = prev?;

	let prev_is_insert = is_point_range(&prev.range);
	let prev_is_delete = prev.new_text.is_empty() && !prev_is_insert;
	let curr_is_insert = is_point_range(&curr.range);
	let curr_is_delete = curr.new_text.is_empty() && !curr_is_insert;

	// Case 1: Insert + Insert at consecutive positions
	// A inserts "foo" at (1,5), B inserts "bar" at (1,8) -> insert "foobar" at (1,5)
	if prev_is_insert && curr_is_insert {
		let prev_end = advance_position(&prev.range.start, &prev.new_text);
		if positions_equal(&prev_end, &curr.range.start) {
			return Some(LspDocumentChange {
				range: prev.range,
				new_text: format!("{}{}", prev.new_text, curr.new_text),
			});
		}
	}

	// Case 2: Delete + Insert at same position (replacement)
	// A deletes (1,5)-(1,10), B inserts at (1,5) -> replace (1,5)-(1,10) with B's text
	if prev_is_delete && curr_is_insert && positions_equal(&prev.range.start, &curr.range.start) {
		return Some(LspDocumentChange {
			range: prev.range,
			new_text: curr.new_text.clone(),
		});
	}

	// Case 3: Delete + Delete at same position
	// After A deletes, positions shift. If B deletes starting at A's start position,
	// it's deleting text that was originally adjacent.
	// A deletes (1,5)-(1,10), B deletes (1,5)-(1,8) -> delete (1,5)-(1,13)
	if prev_is_delete && curr_is_delete && positions_equal(&prev.range.start, &curr.range.start) {
		let prev_deleted_chars = range_char_count(&prev.range);
		let curr_deleted_chars = range_char_count(&curr.range);
		return Some(LspDocumentChange {
			range: LspRange::new(
				prev.range.start,
				advance_position_by(&prev.range.start, prev_deleted_chars + curr_deleted_chars),
			),
			new_text: String::new(),
		});
	}

	// Case 4: Insert + Delete of just-inserted text (undo insert)
	// A inserts "foo" at (1,5), B deletes (1,5)-(1,8) -> empty change or partial
	if prev_is_insert && curr_is_delete && positions_equal(&prev.range.start, &curr.range.start) {
		let inserted_len = count_chars(&prev.new_text);
		let deleted_chars = range_char_count(&curr.range);

		if deleted_chars == inserted_len {
			// Complete undo - return empty insert (effectively no-op)
			return Some(LspDocumentChange {
				range: LspRange::point(prev.range.start),
				new_text: String::new(),
			});
		} else if deleted_chars < inserted_len {
			// Partial delete of inserted text - keep remaining insert
			let remaining = skip_chars(&prev.new_text, deleted_chars as usize);
			return Some(LspDocumentChange {
				range: LspRange::point(prev.range.start),
				new_text: remaining,
			});
		}
		// deleted_chars > inserted_len: deleting more than was inserted,
		// this extends into original text, don't merge
	}

	None
}

/// Returns true if the range is a point (start == end).
fn is_point_range(range: &LspRange) -> bool {
	positions_equal(&range.start, &range.end)
}

/// Returns true if two positions are equal.
fn positions_equal(a: &LspPosition, b: &LspPosition) -> bool {
	a.line == b.line && a.character == b.character
}

/// Advances a position by the characters in text (handling newlines).
fn advance_position(pos: &LspPosition, text: &str) -> LspPosition {
	let mut line = pos.line;
	let mut character = pos.character;

	for ch in text.chars() {
		if ch == '\n' {
			line += 1;
			character = 0;
		} else {
			character += 1;
		}
	}

	LspPosition::new(line, character)
}

/// Advances a position by a number of characters (assuming no newlines for simplicity).
///
/// This is used for delete merging on the same line.
fn advance_position_by(pos: &LspPosition, chars: u32) -> LspPosition {
	LspPosition::new(pos.line, pos.character + chars)
}

/// Counts the characters in a range (assuming same line for simplicity).
fn range_char_count(range: &LspRange) -> u32 {
	if range.start.line == range.end.line {
		range.end.character - range.start.character
	} else {
		// Multi-line ranges are more complex; don't try to merge these
		u32::MAX
	}
}

/// Counts characters in a string.
fn count_chars(s: &str) -> u32 {
	s.chars().count() as u32
}

/// Skips n characters from the start of a string.
fn skip_chars(s: &str, n: usize) -> String {
	s.chars().skip(n).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn pos(line: u32, character: u32) -> LspPosition {
		LspPosition::new(line, character)
	}

	fn point(line: u32, character: u32) -> LspRange {
		LspRange::point(pos(line, character))
	}

	fn range(sl: u32, sc: u32, el: u32, ec: u32) -> LspRange {
		LspRange::new(pos(sl, sc), pos(el, ec))
	}

	#[test]
	fn test_no_changes() {
		let changes = vec![];
		assert_eq!(coalesce_changes(changes), vec![]);
	}

	#[test]
	fn test_single_change() {
		let changes = vec![LspDocumentChange {
			range: point(0, 0),
			new_text: "hello".to_string(),
		}];
		let result = coalesce_changes(changes.clone());
		assert_eq!(result, changes);
	}

	#[test]
	fn test_consecutive_inserts_merged() {
		let changes = vec![
			LspDocumentChange {
				range: point(1, 5),
				new_text: "foo".to_string(),
			},
			LspDocumentChange {
				range: point(1, 8), // 5 + 3 = 8
				new_text: "bar".to_string(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].range, point(1, 5));
		assert_eq!(result[0].new_text, "foobar");
	}

	#[test]
	fn test_non_consecutive_inserts_not_merged() {
		let changes = vec![
			LspDocumentChange {
				range: point(1, 5),
				new_text: "foo".to_string(),
			},
			LspDocumentChange {
				range: point(1, 10), // gap between 8 and 10
				new_text: "bar".to_string(),
			},
		];
		let result = coalesce_changes(changes.clone());
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn test_delete_plus_insert_becomes_replace() {
		let changes = vec![
			LspDocumentChange {
				range: range(1, 5, 1, 10),
				new_text: String::new(),
			},
			LspDocumentChange {
				range: point(1, 5),
				new_text: "new".to_string(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].range, range(1, 5, 1, 10));
		assert_eq!(result[0].new_text, "new");
	}

	#[test]
	fn test_consecutive_deletes_merged() {
		let changes = vec![
			LspDocumentChange {
				range: range(1, 5, 1, 10),
				new_text: String::new(),
			},
			LspDocumentChange {
				range: range(1, 5, 1, 8), // deletes 3 more chars at same position
				new_text: String::new(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].range, range(1, 5, 1, 13)); // 5 + 5 + 3 = 13
		assert_eq!(result[0].new_text, "");
	}

	#[test]
	fn test_insert_then_complete_delete_cancels() {
		let changes = vec![
			LspDocumentChange {
				range: point(1, 5),
				new_text: "foo".to_string(),
			},
			LspDocumentChange {
				range: range(1, 5, 1, 8), // delete exactly "foo"
				new_text: String::new(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].range, point(1, 5));
		assert_eq!(result[0].new_text, "");
	}

	#[test]
	fn test_insert_then_partial_delete() {
		let changes = vec![
			LspDocumentChange {
				range: point(1, 5),
				new_text: "foobar".to_string(),
			},
			LspDocumentChange {
				range: range(1, 5, 1, 8), // delete "foo", keep "bar"
				new_text: String::new(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].range, point(1, 5));
		assert_eq!(result[0].new_text, "bar");
	}

	#[test]
	fn test_three_consecutive_inserts() {
		let changes = vec![
			LspDocumentChange {
				range: point(0, 0),
				new_text: "a".to_string(),
			},
			LspDocumentChange {
				range: point(0, 1),
				new_text: "b".to_string(),
			},
			LspDocumentChange {
				range: point(0, 2),
				new_text: "c".to_string(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].new_text, "abc");
	}

	#[test]
	fn test_insert_with_newline() {
		let changes = vec![
			LspDocumentChange {
				range: point(0, 5),
				new_text: "foo\n".to_string(),
			},
			LspDocumentChange {
				range: point(1, 0), // after newline, we're at line 1, col 0
				new_text: "bar".to_string(),
			},
		];
		let result = coalesce_changes(changes);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].new_text, "foo\nbar");
	}
}
