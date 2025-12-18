use ropey::RopeSlice;
use smallvec::{SmallVec, smallvec};

use crate::range::{Direction, Range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
	ranges: SmallVec<[Range; 1]>,
	primary_index: usize,
}

impl Selection {
	pub fn new(ranges: SmallVec<[Range; 1]>, primary_index: usize) -> Self {
		debug_assert!(!ranges.is_empty());
		debug_assert!(primary_index < ranges.len());

		let mut sel = Self {
			ranges,
			primary_index,
		};
		sel.normalize();
		sel
	}

	pub fn from_vec(ranges: Vec<Range>, primary_index: usize) -> Self {
		Self::new(ranges.into_iter().collect(), primary_index)
	}

	pub fn single(anchor: usize, head: usize) -> Self {
		Self {
			ranges: smallvec![Range::new(anchor, head)],
			primary_index: 0,
		}
	}

	pub fn point(pos: usize) -> Self {
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

	pub fn len(&self) -> usize {
		self.ranges.len()
	}

	pub fn is_empty(&self) -> bool {
		self.ranges.is_empty()
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

	pub fn transform<F>(&self, f: F) -> Self
	where
		F: FnMut(&Range) -> Range,
	{
		Self::new(self.ranges.iter().map(f).collect(), self.primary_index)
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

	pub fn merge_overlaps_and_adjacent(&mut self) {
		if self.ranges.len() <= 1 {
			return;
		}

		let primary = self.ranges[self.primary_index];
		self.ranges.sort_by_key(|r| r.from());

		let mut merged: SmallVec<[Range; 1]> = SmallVec::new();
		let mut primary_index = 0;

		for range in &self.ranges {
			if let Some(last) = merged.last_mut()
				&& (last.overlaps(range) || last.to() == range.from())
			{
				*last = last.merge(range);
				if *range == primary || last.contains(primary.from()) {
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

	fn normalize(&mut self) {
		if self.ranges.len() <= 1 {
			return;
		}

		let primary = self.ranges[self.primary_index];

		self.ranges.sort_by_key(|r| r.from());

		let mut merged: SmallVec<[Range; 1]> = SmallVec::new();
		let mut primary_index = 0;

		for range in &self.ranges {
			if let Some(last) = merged.last_mut()
				&& last.overlaps(range)
			{
				*last = last.merge(range);
				if *range == primary || last.contains(primary.from()) {
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
		Self::new(
			self.ranges
				.into_iter()
				.map(|r| r.grapheme_aligned(text))
				.collect(),
			self.primary_index,
		)
	}

	pub fn contains(&self, pos: usize) -> bool {
		self.ranges.iter().any(|r| r.contains(pos))
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
		let ranges = smallvec![Range::new(0, 5), Range::new(10, 15), Range::new(20, 25)];
		let sel = Selection::new(ranges, 1);
		assert_eq!(sel.len(), 3);
		assert_eq!(sel.primary(), Range::new(10, 15));
	}

	#[test]
	fn test_merge_overlapping() {
		let ranges = smallvec![Range::new(0, 10), Range::new(5, 15)];
		let sel = Selection::new(ranges, 0);
		assert_eq!(sel.len(), 1);
		assert_eq!(sel.ranges()[0].from(), 0);
		assert_eq!(sel.ranges()[0].to(), 15);
	}

	#[test]
	fn test_merge_duplicate_cursors() {
		let ranges = smallvec![Range::point(5), Range::point(5)];
		let sel = Selection::new(ranges, 0);
		assert_eq!(sel.len(), 1);
		assert_eq!(sel.primary(), Range::point(5));
	}

	#[test]
	fn test_do_not_merge_adjacent() {
		let ranges = smallvec![Range::new(0, 5), Range::new(5, 10)];
		let sel = Selection::new(ranges, 0);
		assert_eq!(sel.len(), 2);
		assert_eq!(sel.ranges()[0].from(), 0);
		assert_eq!(sel.ranges()[0].to(), 5);
		assert_eq!(sel.ranges()[1].from(), 5);
		assert_eq!(sel.ranges()[1].to(), 10);
	}

	#[test]
	fn test_no_merge_gap() {
		let ranges = smallvec![Range::new(0, 5), Range::new(6, 10)];
		let sel = Selection::new(ranges, 0);
		assert_eq!(sel.len(), 2);
	}

	#[test]
	fn test_merge_overlaps_and_adjacent_command() {
		let ranges = smallvec![Range::new(0, 5), Range::new(5, 10), Range::new(12, 14)];
		let mut sel = Selection::new(ranges, 0);
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
