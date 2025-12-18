use ropey::RopeSlice;

use crate::graphemes::{ensure_grapheme_boundary_next, ensure_grapheme_boundary_prev};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
	Forward,
	Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
	pub anchor: usize,
	pub head: usize,
}

impl Range {
	pub fn new(anchor: usize, head: usize) -> Self {
		Self { anchor, head }
	}

	pub fn point(pos: usize) -> Self {
		Self::new(pos, pos)
	}

	#[inline]
	pub fn from(&self) -> usize {
		std::cmp::min(self.anchor, self.head)
	}

	#[inline]
	pub fn to(&self) -> usize {
		std::cmp::max(self.anchor, self.head)
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.to() - self.from()
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.anchor == self.head
	}

	#[inline]
	pub fn direction(&self) -> Direction {
		if self.head < self.anchor {
			Direction::Backward
		} else {
			Direction::Forward
		}
	}

	pub fn flip(&self) -> Self {
		Self {
			anchor: self.head,
			head: self.anchor,
		}
	}

	pub fn with_direction(self, direction: Direction) -> Self {
		if self.direction() == direction {
			self
		} else {
			self.flip()
		}
	}

	pub fn map(self, mut f: impl FnMut(usize) -> usize) -> Self {
		Self {
			anchor: f(self.anchor),
			head: f(self.head),
		}
	}

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

	pub fn contains(&self, pos: usize) -> bool {
		pos >= self.from() && pos < self.to()
	}

	pub fn overlaps(&self, other: &Range) -> bool {
		if self.from() < other.to() && other.from() < self.to() {
			return true;
		}

		self.is_empty() && other.is_empty() && self.from() == other.from()
	}

	pub fn merge(&self, other: &Range) -> Self {
		let from = std::cmp::min(self.from(), other.from());
		let to = std::cmp::max(self.to(), other.to());

		if self.direction() == Direction::Forward {
			Self::new(from, to)
		} else {
			Self::new(to, from)
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
		assert_eq!(r.from(), 5);
		assert_eq!(r.to(), 10);
		assert_eq!(r.len(), 5);
		assert!(!r.is_empty());
		assert_eq!(r.direction(), Direction::Forward);
	}

	#[test]
	fn test_range_backward() {
		let r = Range::new(10, 5);
		assert_eq!(r.from(), 5);
		assert_eq!(r.to(), 10);
		assert_eq!(r.direction(), Direction::Backward);
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
		assert_eq!(merged.from(), 5);
		assert_eq!(merged.to(), 15);
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
