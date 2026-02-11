use ropey::RopeSlice;
use smallvec::{SmallVec, smallvec};

use crate::range::{CharIdx, Direction, Range};

#[cfg(test)]
mod tests;

/// A set of non-overlapping ranges with a designated primary.
///
/// A selection always contains at least one range. The primary range
/// is used for cursor positioning and scroll following.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
	/// The collection of selection ranges (always non-empty).
	ranges: SmallVec<[Range; 1]>,
	/// Index of the primary range within `ranges`.
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

		let mut sel = Self { ranges, primary_index: 0 };
		sel.normalize();
		sel
	}

	/// Creates a selection from a vector of ranges.
	///
	/// # Panics
	///
	/// Panics if `ranges` is empty.
	pub fn from_vec(ranges: Vec<Range>, primary_index: usize) -> Self {
		assert!(!ranges.is_empty(), "Selection cannot be empty");
		assert!(
			primary_index < ranges.len(),
			"primary_index ({primary_index}) out of bounds for {} ranges",
			ranges.len()
		);

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

	/// Creates a single-range selection.
	pub fn single(anchor: CharIdx, head: CharIdx) -> Self {
		Self {
			ranges: smallvec![Range::new(anchor, head)],
			primary_index: 0,
		}
	}

	/// Creates a point selection (zero-width cursor).
	pub fn point(pos: CharIdx) -> Self {
		Self::single(pos, pos)
	}

	/// Returns the primary range.
	pub fn primary(&self) -> Range {
		self.ranges[self.primary_index]
	}

	/// Returns the index of the primary range.
	pub fn primary_index(&self) -> usize {
		self.primary_index
	}

	/// Sets the primary range by index.
	pub fn set_primary(&mut self, index: usize) {
		assert!(
			index < self.ranges.len(),
			"primary index ({index}) out of bounds for {} ranges",
			self.ranges.len()
		);
		self.primary_index = index;
	}

	/// Returns all ranges as a slice.
	pub fn ranges(&self) -> &[Range] {
		&self.ranges
	}

	/// Returns the number of ranges in this selection.
	///
	/// This is always at least 1 (Selection cannot be empty).
	#[allow(clippy::len_without_is_empty, reason = "Selection is never empty by design")]
	pub fn len(&self) -> usize {
		self.ranges.len()
	}

	/// Iterates over all ranges.
	pub fn iter(&self) -> impl Iterator<Item = &Range> {
		self.ranges.iter()
	}

	/// Adds a new range to the selection.
	pub fn push(&mut self, range: Range) {
		self.ranges.push(range);
		self.normalize();
	}

	/// Replaces the range at the given index.
	pub fn replace(&mut self, index: usize, range: Range) {
		self.ranges[index] = range;
		self.normalize();
	}

	/// Transforms all ranges using the given function, returning a new selection.
	pub fn transform<F>(&self, mut f: F) -> Self
	where
		F: FnMut(&Range) -> Range,
	{
		let primary = f(&self.primary());
		let others = self.ranges.iter().enumerate().filter(|&(i, _)| i != self.primary_index).map(|(_, r)| f(r));

		Self::new(primary, others)
	}

	/// Transforms all ranges in place using the given function.
	pub fn transform_mut<F>(&mut self, mut f: F)
	where
		F: FnMut(&mut Range),
	{
		for range in &mut self.ranges {
			f(range);
		}
		self.normalize();
	}

	/// Transforms ranges with a fallible function, filtering out `None` results.
	///
	/// Returns `None` if all ranges are filtered out. Remaps primary index automatically.
	pub fn try_filter_transform<F>(&self, mut f: F) -> Option<Self>
	where
		F: FnMut(&Range) -> Option<Range>,
	{
		let primary_sel_idx = self.primary_index;
		let mut ranges = Vec::new();
		let mut new_primary_index = 0;

		for (idx, range) in self.ranges.iter().enumerate() {
			if let Some(new_range) = f(range) {
				if idx == primary_sel_idx {
					new_primary_index = ranges.len();
				}
				ranges.push(new_range);
			}
		}

		if ranges.is_empty() {
			None
		} else {
			Some(Self::from_vec(ranges, new_primary_index))
		}
	}

	/// Merge overlapping AND adjacent ranges.
	///
	/// Unlike `normalize()` (which only merges overlapping ranges), this also
	/// merges ranges that are adjacent (touching but not overlapping).
	///
	/// # 1-Cell Model Adjacency
	///
	/// In the 1-cell inclusive model, ranges `[a, b]` and `[c, d]` are adjacent if
	/// `b + 1 == c`. This means there is no gap between the last cell of the first range
	/// and the first cell of the second range.
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
				&& (last.overlaps(range) || last.max() + 1 == range.min())
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

	/// Returns a new selection with all ranges aligned to grapheme boundaries.
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

	/// Returns true if any range contains the given position.
	pub fn contains(&self, pos: CharIdx) -> bool {
		self.ranges.iter().any(|r: &Range| r.contains(pos))
	}

	/// Returns the direction of the primary range.
	pub fn direction(&self) -> Direction {
		self.primary().direction()
	}

	/// Rotates the primary selection to the next range.
	pub fn rotate_forward(&mut self) {
		if self.ranges.len() > 1 {
			self.primary_index = (self.primary_index + 1) % self.ranges.len();
		}
	}

	/// Rotates the primary selection to the previous range.
	pub fn rotate_backward(&mut self) {
		if self.ranges.len() > 1 {
			self.primary_index = (self.primary_index + self.ranges.len() - 1) % self.ranges.len();
		}
	}

	/// Removes the primary range (if more than one range exists).
	pub fn remove_primary(&mut self) {
		if self.ranges.len() > 1 {
			self.ranges.remove(self.primary_index);
			self.primary_index = self.primary_index.min(self.ranges.len().saturating_sub(1));
		}
	}

	/// Clamps all ranges to `[0, max_char]`.
	pub fn clamp(&mut self, max_char: CharIdx) {
		for range in &mut self.ranges {
			*range = range.clamp(max_char);
		}
		self.normalize();
	}

	/// Returns `true` if all ranges are within valid document boundaries.
	///
	/// A selection is considered in-bounds if:
	/// - The primary index is valid for the current ranges vector.
	/// - All range endpoints are within document bounds `[0, len]`.
	/// - The extent `[from, to)` is allowed to be `[0, 1)` even for empty documents
	///   to represent the 1-cell cursor (consumers should clamp this to the actual length).
	#[inline]
	pub fn is_in_bounds(&self, len: CharIdx) -> bool {
		if self.primary_index >= self.ranges.len() {
			return false;
		}
		self.ranges.iter().all(|r| {
			let from = r.from();
			let to = r.to();
			from <= len && to <= len + 1 && r.head <= len && r.anchor <= len
		})
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
