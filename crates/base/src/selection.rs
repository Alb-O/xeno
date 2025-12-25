use ropey::RopeSlice;
use smallvec::{SmallVec, smallvec};

use crate::range::{CharIdx, Direction, Range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
	ranges: SmallVec<[Range; 1]>,
	primary_index: usize,
}

impl Selection {
	/// Create a new selection with at least one range.
	///
	/// The `primary` range is the one that will be used for most operations
	/// (like scrolling or cursor-based motions). Additional ranges can be
	/// provided via the `others` iterator.
	pub fn new(primary: Range, others: impl IntoIterator<Item = Range>) -> Self {
		let mut ranges: SmallVec<[Range; 1]> = smallvec![primary];
		ranges.extend(others);

		let mut sel = Self {
			ranges,
			primary_index: 0,
		};
		sel.normalize();
		sel
	}

	pub fn from_vec(ranges: Vec<Range>, primary_index: usize) -> Self {
		assert!(!ranges.is_empty(), "Selection cannot be empty");
		debug_assert!(primary_index < ranges.len());

		// We need to preserve which one was primary before normalization
		let primary = ranges[primary_index];

		let mut sel = Self {
			ranges: ranges.into_iter().collect(),
			primary_index: 0,
		};

		// Re-find the primary after putting it into SmallVec
		sel.primary_index = sel.ranges.iter().position(|&r| r == primary).unwrap_or(0);

		sel.normalize();
		sel
	}

	pub fn single(anchor: CharIdx, head: CharIdx) -> Self {
		Self {
			ranges: smallvec![Range::new(anchor, head)],
			primary_index: 0,
		}
	}

	pub fn point(pos: CharIdx) -> Self {
		Self::single(pos, pos)
	}

	pub fn primary(&self) -> Range {
		self.ranges[self.primary_index]
	}

	pub fn primary_index(&self) -> usize {
		self.primary_index
	}

	pub fn set_primary(&mut self, index: usize) {
		debug_assert!(index < self.ranges.len());
		self.primary_index = index;
	}

	pub fn ranges(&self) -> &[Range] {
		&self.ranges
	}

	/// Returns the number of ranges in this selection.
	///
	/// This is always at least 1 (Selection cannot be empty).
	#[allow(clippy::len_without_is_empty)]
	pub fn len(&self) -> usize {
		self.ranges.len()
	}

	pub fn iter(&self) -> impl Iterator<Item = &Range> {
		self.ranges.iter()
	}

	pub fn push(&mut self, range: Range) {
		self.ranges.push(range);
		self.normalize();
	}

	pub fn replace(&mut self, index: usize, range: Range) {
		self.ranges[index] = range;
		self.normalize();
	}

	pub fn transform<F>(&self, mut f: F) -> Self
	where
		F: FnMut(&Range) -> Range,
	{
		let primary = f(&self.primary());
		let others = self
			.ranges
			.iter()
			.enumerate()
			.filter(|&(i, _)| i != self.primary_index)
			.map(|(_, r)| f(r));

		Self::new(primary, others)
	}

	pub fn transform_mut<F>(&mut self, mut f: F)
	where
		F: FnMut(&mut Range),
	{
		for range in &mut self.ranges {
			f(range);
		}
		self.normalize();
	}

	/// Merge overlapping AND adjacent ranges.
	///
	/// Unlike `normalize()` (which only merges overlapping ranges), this also
	/// merges ranges that are adjacent (touching but not overlapping).
	/// For example, `[0, 5)` and `[5, 10)` would be merged into `[0, 10)`.
	///
	/// Use this when you want to combine all contiguous selections into
	/// single ranges (e.g., for visual selection operations).
	pub fn merge_overlaps_and_adjacent(&mut self) {
		if self.ranges.len() <= 1 {
			return;
		}

		let primary = self.ranges[self.primary_index];
		self.ranges.sort_by_key(|r: &Range| r.min());

		let mut merged: SmallVec<[Range; 1]> = SmallVec::new();
		let mut primary_index = 0;

		for range in &self.ranges {
			if let Some(last) = merged.last_mut()
				&& (last.overlaps(range) || last.max() == range.min())
			{
				let old_last = *last;
				*last = last.merge(range);
				if *range == primary || old_last == primary || last.contains(primary.min()) {
					primary_index = merged.len() - 1;
				}
				continue;
			}

			if *range == primary {
				primary_index = merged.len();
			}
			merged.push(*range);
		}

		self.ranges = merged;
		self.primary_index = primary_index.min(self.ranges.len().saturating_sub(1));
	}

	/// Normalize the selection by sorting ranges and merging overlaps.
	///
	/// This is the canonical normalization that is automatically called after
	/// most operations. It merges ranges that overlap but keeps adjacent
	/// ranges separate. For example, `[0, 5)` and `[5, 10)` remain separate.
	///
	/// If you want to also merge adjacent ranges, use `merge_overlaps_and_adjacent()`.
	fn normalize(&mut self) {
		if self.ranges.len() <= 1 {
			return;
		}

		let primary = self.ranges[self.primary_index];

		self.ranges.sort_by_key(|r: &Range| r.min());

		let mut merged: SmallVec<[Range; 1]> = SmallVec::new();
		let mut primary_index = 0;

		for range in &self.ranges {
			if let Some(last) = merged.last_mut()
				&& last.overlaps(range)
			{
				let old_last = *last;
				*last = last.merge(range);
				if *range == primary || old_last == primary || last.contains(primary.min()) {
					primary_index = merged.len() - 1;
				}
				continue;
			}

			if *range == primary {
				primary_index = merged.len();
			}
			merged.push(*range);
		}

		self.ranges = merged;
		self.primary_index = primary_index.min(self.ranges.len().saturating_sub(1));
	}

	pub fn grapheme_aligned(self, text: RopeSlice) -> Self {
		let primary = self.primary().grapheme_aligned(text);
		let others = self
			.ranges
			.iter()
			.enumerate()
			.filter(|&(i, _)| i != self.primary_index)
			.map(|(_, r)| r.grapheme_aligned(text));

		Self::new(primary, others)
	}

	pub fn contains(&self, pos: CharIdx) -> bool {
		self.ranges.iter().any(|r: &Range| r.contains(pos))
	}

	pub fn direction(&self) -> Direction {
		self.primary().direction()
	}

	pub fn rotate_forward(&mut self) {
		if self.ranges.len() > 1 {
			self.primary_index = (self.primary_index + 1) % self.ranges.len();
		}
	}

	pub fn rotate_backward(&mut self) {
		if self.ranges.len() > 1 {
			self.primary_index = (self.primary_index + self.ranges.len() - 1) % self.ranges.len();
		}
	}

	pub fn remove_primary(&mut self) {
		if self.ranges.len() > 1 {
			self.ranges.remove(self.primary_index);
			self.primary_index = self.primary_index.min(self.ranges.len().saturating_sub(1));
		}
	}
}

impl Default for Selection {
	fn default() -> Self {
		Self::point(0)
	}
}

impl From<Range> for Selection {
	fn from(range: Range) -> Self {
		Self {
			ranges: smallvec![range],
			primary_index: 0,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_single_selection() {
		let sel = Selection::single(5, 10);
		assert_eq!(sel.len(), 1);
		assert_eq!(sel.primary(), Range::new(5, 10));
	}

	#[test]
	fn test_point_selection() {
		let sel = Selection::point(5);
		assert_eq!(sel.len(), 1);
		assert!(sel.primary().is_empty());
	}

	#[test]
	fn test_multi_selection() {
		let primary = Range::new(10, 15);
		let others = vec![Range::new(0, 5), Range::new(20, 25)];
		let sel = Selection::new(primary, others);
		assert_eq!(sel.len(), 3);
		assert_eq!(sel.primary(), Range::new(10, 15));
	}

	#[test]
	fn test_merge_overlapping() {
		let primary = Range::new(0, 10);
		let others = vec![Range::new(5, 15)];
		let sel = Selection::new(primary, others);
		assert_eq!(sel.len(), 1);
		assert_eq!(sel.ranges()[0].min(), 0);
		assert_eq!(sel.ranges()[0].max(), 15);
	}

	#[test]
	fn test_merge_duplicate_cursors() {
		let primary = Range::point(5);
		let others = vec![Range::point(5)];
		let sel = Selection::new(primary, others);
		assert_eq!(sel.len(), 1);
		assert_eq!(sel.primary(), Range::point(5));
	}

	#[test]
	fn test_do_not_merge_adjacent() {
		let primary = Range::new(0, 5);
		let others = vec![Range::new(5, 10)];
		let sel = Selection::new(primary, others);
		assert_eq!(sel.len(), 2);
		assert_eq!(sel.ranges()[0].min(), 0);
		assert_eq!(sel.ranges()[0].max(), 5);
		assert_eq!(sel.ranges()[1].min(), 5);
		assert_eq!(sel.ranges()[1].max(), 10);
	}

	#[test]
	fn test_no_merge_gap() {
		let primary = Range::new(0, 5);
		let others = vec![Range::new(6, 10)];
		let sel = Selection::new(primary, others);
		assert_eq!(sel.len(), 2);
	}

	#[test]
	fn test_merge_overlaps_and_adjacent_command() {
		let primary = Range::new(0, 5);
		let others = vec![Range::new(5, 10), Range::new(12, 14)];
		let mut sel = Selection::new(primary, others);
		sel.merge_overlaps_and_adjacent();
		assert_eq!(sel.len(), 2);
		assert_eq!(sel.ranges()[0], Range::new(0, 10));
		assert_eq!(sel.ranges()[1], Range::new(12, 14));
	}

	#[test]
	fn test_transform() {
		let sel = Selection::single(5, 10);
		let transformed = sel.transform(|r| Range::new(r.anchor + 1, r.head + 1));
		assert_eq!(transformed.primary(), Range::new(6, 11));
	}

	#[test]
	fn test_contains() {
		let sel = Selection::single(5, 10);
		assert!(!sel.contains(4));
		assert!(sel.contains(5));
		assert!(sel.contains(7));
		assert!(!sel.contains(10));
	}
}
