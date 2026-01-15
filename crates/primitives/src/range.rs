use ropey::RopeSlice;

use crate::graphemes::{ensure_grapheme_boundary_next, ensure_grapheme_boundary_prev};

/// Selection direction (anchor to head).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
	/// Head is after anchor (normal selection).
	Forward,
	/// Head is before anchor (reverse selection).
	Backward,
}

/// A position in the text, measured in characters (not bytes).
///
/// This is the canonical coordinate space for Xeno.
pub type CharIdx = usize;

/// A length or count in the text, measured in characters (not bytes).
///
/// This is distinct from CharIdx to avoid accidentally passing an index
/// where a length is expected or vice versa.
pub type CharLen = usize;

/// A text range defined by anchor and head positions.
///
/// The anchor is the fixed end, and the head moves during selection extension.
/// For a forward selection, head > anchor. For backward, head < anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
	/// The fixed end of the range.
	pub anchor: CharIdx,
	/// The moving end of the range (cursor position).
	pub head: CharIdx,
}

impl Range {
	/// Creates a new range from anchor to head.
	pub fn new(anchor: CharIdx, head: CharIdx) -> Self {
		Self { anchor, head }
	}

	/// Creates a zero-width range (cursor) at the given position.
	pub fn point(pos: CharIdx) -> Self {
		Self::new(pos, pos)
	}

	/// Returns the smaller of anchor and head.
	#[inline]
	pub fn min(&self) -> CharIdx {
		std::cmp::min(self.anchor, self.head)
	}

	/// Returns the larger of anchor and head.
	#[inline]
	pub fn max(&self) -> CharIdx {
		std::cmp::max(self.anchor, self.head)
	}

	/// Returns the start of the selection extent (inclusive).
	///
	/// For both forward and backward selections, this is `min()`.
	#[inline]
	pub fn from(&self) -> CharIdx {
		self.min()
	}

	/// Returns the end of the selection extent (exclusive).
	///
	/// For backward selections, this includes the anchor character by
	/// returning `anchor + 1`. This ensures the character under the cursor
	/// when selection started is included.
	#[inline]
	pub fn to(&self) -> CharIdx {
		if self.direction() == Direction::Backward {
			self.anchor + 1
		} else {
			self.max()
		}
	}

	/// Returns the length of the range in characters.
	#[inline]
	pub fn len(&self) -> CharLen {
		self.to() - self.from()
	}

	/// Returns true if anchor equals head (zero-width cursor).
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.anchor == self.head
	}

	/// Returns the direction of this range.
	#[inline]
	pub fn direction(&self) -> Direction {
		if self.head < self.anchor {
			Direction::Backward
		} else {
			Direction::Forward
		}
	}

	/// Returns a new range with anchor and head swapped.
	pub fn flip(&self) -> Self {
		Self {
			anchor: self.head,
			head: self.anchor,
		}
	}

	/// Returns a range with the specified direction, flipping if needed.
	pub fn with_direction(self, direction: Direction) -> Self {
		if self.direction() == direction {
			self
		} else {
			self.flip()
		}
	}

	/// Applies a function to both anchor and head.
	pub fn map(self, mut f: impl FnMut(CharIdx) -> CharIdx) -> Self {
		Self {
			anchor: f(self.anchor),
			head: f(self.head),
		}
	}

	/// Returns a range with positions aligned to grapheme boundaries.
	pub fn grapheme_aligned(self, text: RopeSlice) -> Self {
		let anchor = if self.anchor == 0 || self.anchor == text.len_chars() {
			self.anchor
		} else if self.direction() == Direction::Forward {
			ensure_grapheme_boundary_prev(text, self.anchor)
		} else {
			ensure_grapheme_boundary_next(text, self.anchor)
		};

		let head = if self.head == 0 || self.head == text.len_chars() {
			self.head
		} else if self.direction() == Direction::Forward {
			ensure_grapheme_boundary_next(text, self.head)
		} else {
			ensure_grapheme_boundary_prev(text, self.head)
		};

		Self { anchor, head }
	}

	/// Returns true if the position is within the range (exclusive of max).
	pub fn contains(&self, pos: CharIdx) -> bool {
		pos >= self.min() && pos < self.max()
	}

	/// Returns true if this range overlaps with another.
	pub fn overlaps(&self, other: &Range) -> bool {
		if self.min() < other.max() && other.min() < self.max() {
			return true;
		}

		self.is_empty() && other.is_empty() && self.min() == other.min()
	}

	/// Merges two ranges, preserving direction of self.
	pub fn merge(&self, other: &Range) -> Self {
		let from = std::cmp::min(self.min(), other.min());
		let to = std::cmp::max(self.max(), other.max());

		if self.direction() == Direction::Forward {
			Self::new(from, to)
		} else {
			Self::new(to, from)
		}
	}

	/// Clamps anchor and head to `[0, max_char]`.
	pub fn clamp(&self, max_char: CharIdx) -> Self {
		Self {
			anchor: self.anchor.min(max_char),
			head: self.head.min(max_char),
		}
	}
}

impl Default for Range {
	fn default() -> Self {
		Self::point(0)
	}
}

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_range_basics() {
		let r = Range::new(5, 10);
		assert_eq!(r.min(), 5);
		assert_eq!(r.max(), 10);
		assert_eq!(r.len(), 5);
		assert!(!r.is_empty());
		assert_eq!(r.direction(), Direction::Forward);
	}

	#[test]
	fn test_range_backward() {
		let r = Range::new(10, 5);
		assert_eq!(r.min(), 5);
		assert_eq!(r.max(), 10);
		assert_eq!(r.direction(), Direction::Backward);
	}

	#[test]
	fn test_range_from_to_forward() {
		// Forward selection: anchor=5, head=10
		// Selects characters 5,6,7,8,9 (head position is cursor, not selected)
		let r = Range::new(5, 10);
		assert_eq!(r.from(), 5);
		assert_eq!(r.to(), 10);
		assert_eq!(r.len(), 5);
	}

	#[test]
	fn test_range_from_to_backward() {
		// Backward selection: anchor=10, head=5
		// Selects characters 5,6,7,8,9,10 (anchor char IS selected)
		let r = Range::new(10, 5);
		assert_eq!(r.from(), 5);
		assert_eq!(r.to(), 11); // anchor + 1 to include anchor char
		assert_eq!(r.len(), 6);
	}

	#[test]
	fn test_range_flip() {
		let r = Range::new(5, 10);
		let flipped = r.flip();
		assert_eq!(flipped.anchor, 10);
		assert_eq!(flipped.head, 5);
	}

	#[test]
	fn test_range_point() {
		let r = Range::point(5);
		assert!(r.is_empty());
		assert_eq!(r.anchor, 5);
		assert_eq!(r.head, 5);
	}

	#[test]
	fn test_range_contains() {
		let r = Range::new(5, 10);
		assert!(!r.contains(4));
		assert!(r.contains(5));
		assert!(r.contains(7));
		assert!(!r.contains(10));
	}

	#[test]
	fn test_range_overlaps() {
		let r1 = Range::new(5, 10);
		let r2 = Range::new(8, 15);
		let r3 = Range::new(10, 15);

		assert!(r1.overlaps(&r2));
		assert!(!r1.overlaps(&r3));
	}

	#[test]
	fn test_range_overlaps_same_point() {
		let r1 = Range::point(5);
		let r2 = Range::point(5);

		assert!(r1.overlaps(&r2));
	}

	#[test]
	fn test_range_merge() {
		let r1 = Range::new(5, 10);
		let r2 = Range::new(8, 15);
		let merged = r1.merge(&r2);
		assert_eq!(merged.min(), 5);
		assert_eq!(merged.max(), 15);
	}

	#[test]
	fn test_grapheme_aligned() {
		let text = Rope::from("hello");
		let slice = text.slice(..);
		let r = Range::new(1, 3);
		let aligned = r.grapheme_aligned(slice);
		assert_eq!(aligned.anchor, 1);
		assert_eq!(aligned.head, 3);
	}
}
