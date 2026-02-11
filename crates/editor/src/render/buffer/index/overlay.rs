use std::collections::{HashMap, HashSet};
use std::ops::Range;

use xeno_primitives::Selection;
use xeno_primitives::range::CharIdx;

/// Classification of cursor state for a given document position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorKind {
	/// No cursor at this position.
	None,
	/// The primary (active) cursor.
	Primary,
	/// A secondary cursor in multi-cursor mode.
	Secondary,
	/// Cursor in an unfocused buffer.
	Unfocused,
}

/// Index for efficient overlay queries (cursor, selection) during rendering.
///
/// Pre-computes data structures to answer:
/// - Is there a cursor at this document position?
/// - Is this line/offset within a selection range?
pub struct OverlayIndex {
	/// Set of all cursor head positions.
	pub cursor_heads: HashSet<CharIdx>,
	/// The primary cursor position.
	pub primary_cursor: CharIdx,
	/// Selection ranges grouped by line (line-relative offsets).
	pub selection_by_line: HashMap<usize, Vec<Range<usize>>>,
}

impl OverlayIndex {
	/// Builds an overlay index from the current selection state.
	///
	/// Aggregates all cursor positions and builds a line-indexed selection map.
	/// Selection ranges on each line are automatically sorted and merged to ensure
	/// efficient binary search lookups during rendering.
	///
	/// # Parameters
	///
	/// - `selection`: The current multi-cursor selection.
	/// - `primary_cursor`: The active cursor position.
	/// - `_is_focused`: Unused focus flag (preserved for API compatibility).
	/// - `rope`: Document content for line mapping.
	pub fn new(selection: &Selection, primary_cursor: CharIdx, _is_focused: bool, rope: &xeno_primitives::Rope) -> Self {
		let len = rope.len_chars();
		let primary_cursor = primary_cursor.min(len);

		let mut cursor_heads = HashSet::new();
		for range in selection.ranges() {
			cursor_heads.insert(range.head.min(len));
		}

		let mut selection_by_line: HashMap<usize, Vec<Range<usize>>> = HashMap::new();
		for range in selection.ranges() {
			let from = range.from().min(len);
			let to = range.to().min(len);

			if from == to {
				continue;
			}
			let start_line = rope.char_to_line(from);
			let end_line = rope.char_to_line(to);

			for line_idx in start_line..=end_line {
				let line_start = rope.line_to_char(line_idx);
				let line_end = if line_idx + 1 < rope.len_lines() {
					rope.line_to_char(line_idx + 1)
				} else {
					rope.len_chars()
				};

				let sel_start = from.max(line_start);
				let sel_end = to.min(line_end);

				if sel_start < sel_end {
					selection_by_line
						.entry(line_idx)
						.or_default()
						.push((sel_start - line_start)..(sel_end - line_start));
				}
			}
		}

		for ranges in selection_by_line.values_mut() {
			ranges.sort_by_key(|r| r.start);

			let mut i = 0;
			while i + 1 < ranges.len() {
				if ranges[i].end >= ranges[i + 1].start {
					ranges[i].end = ranges[i].end.max(ranges[i + 1].end);
					ranges.remove(i + 1);
				} else {
					i += 1;
				}
			}
		}

		Self {
			cursor_heads,
			primary_cursor,
			selection_by_line,
		}
	}

	/// Checks if the given line offset is within a selection range.
	///
	/// Uses binary search on merged ranges for O(log n) lookup.
	pub fn in_selection(&self, line_idx: usize, char_off: usize) -> bool {
		let Some(ranges) = self.selection_by_line.get(&line_idx) else {
			return false;
		};

		ranges
			.binary_search_by(|r| {
				if char_off < r.start {
					std::cmp::Ordering::Greater
				} else if char_off >= r.end {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Equal
				}
			})
			.is_ok()
	}

	/// Returns the cursor kind for the given document position.
	///
	/// Returns [`CursorKind::None`] if no cursor is at this position.
	pub fn cursor_kind(&self, doc_pos: CharIdx, is_focused: bool) -> CursorKind {
		if !self.cursor_heads.contains(&doc_pos) {
			return CursorKind::None;
		}

		if !is_focused {
			CursorKind::Unfocused
		} else if doc_pos == self.primary_cursor {
			CursorKind::Primary
		} else {
			CursorKind::Secondary
		}
	}

	/// Checks if any part of the given line segment is covered by a selection.
	///
	/// Used to ensure visual continuity of selection highlights across layout
	/// boundaries like soft-wrap indents.
	pub fn segment_selected(&self, line_idx: usize, start: usize, end: usize) -> bool {
		let Some(ranges) = self.selection_by_line.get(&line_idx) else {
			return false;
		};

		// Check if any selection range on this line overlaps [start, end)
		ranges.iter().any(|r| r.start < end && start < r.end)
	}
}
