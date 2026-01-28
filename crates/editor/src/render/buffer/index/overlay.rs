use std::collections::{HashMap, HashSet};
use std::ops::Range;

use xeno_primitives::Selection;
use xeno_primitives::range::CharIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorKind {
	None,
	Primary,
	Secondary,
	Unfocused,
}

#[derive(Debug, Clone)]
pub struct OverlayIndex {
	pub cursor_heads: HashSet<CharIdx>,
	pub primary_cursor: CharIdx,
	pub selection_by_line: HashMap<usize, Vec<Range<usize>>>,
}

impl OverlayIndex {
	pub fn new(
		selection: &Selection,
		primary_cursor: CharIdx,
		_is_focused: bool,
		rope: &xeno_primitives::Rope, // To map char positions to lines
	) -> Self {
		let mut cursor_heads = HashSet::new();
		for range in selection.ranges() {
			cursor_heads.insert(range.head);
		}

		let mut selection_by_line: HashMap<usize, Vec<Range<usize>>> = HashMap::new();
		for range in selection.ranges() {
			if range.is_empty() {
				continue;
			}

			let from = range.from();
			let to = range.to();
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

		// Sort and merge selection ranges per line
		for ranges in selection_by_line.values_mut() {
			ranges.sort_by_key(|r| r.start);
			// Merging is actually not needed if ranges are non-overlapping in Xeno,
			// but good for robustness.
		}

		Self {
			cursor_heads,
			primary_cursor,
			selection_by_line,
		}
	}

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
}
