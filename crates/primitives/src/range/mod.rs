use ropey::RopeSlice;

use crate::graphemes::{ensure_grapheme_boundary_next, ensure_grapheme_boundary_prev};

#[cfg(test)]
mod tests;

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

	/// Creates a new range from an exclusive interval `[start, end)`.
	///
	/// In the 1-cell minimum model, this maps to an inclusive range `[start, end - 1]`.
	///
	/// # Panics
	///
	/// Panics if `start >= end`, as an exclusive range must contain at least one character cell.
	pub fn from_exclusive(start: CharIdx, end: CharIdx) -> Self {
		assert!(start < end, "exclusive range must have at least one cell");
		Self::new(start, end - 1)
	}

	/// Creates a 1-cell range (cursor) at the given position.
	///
	/// In the 1-cell model, a point selection still selects one character.
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

	/// Returns the end of the selection extent (exclusive of end position).
	///
	/// In the 1-cell minimum model, this is always `max() + 1`, ensuring the
	/// character at the head position is included in operations.
	#[inline]
	pub fn to(&self) -> CharIdx {
		self.max() + 1
	}

	/// Returns the length of the range in characters.
	///
	/// A point selection (anchor == head) has a length of 1.
	#[inline]
	pub fn len(&self) -> CharLen {
		self.to() - self.from()
	}

	/// Returns true if the range is empty.
	///
	/// In the 1-cell minimum model, a range is never empty as it always contains
	/// at least one character cell.
	#[inline]
	pub fn is_empty(&self) -> bool {
		false
	}

	/// Returns true if anchor equals head (point selection).
	///
	/// In the 1-cell minimum model, a point selection still selects one character.
	#[inline]
	pub fn is_point(&self) -> bool {
		self.anchor == self.head
	}

	/// Returns the selection extent [from, to) clamped to document length.
	///
	/// Collapses to [len, len) if selection is beyond EOF.
	pub fn extent_clamped(&self, len: CharIdx) -> (CharIdx, CharIdx) {
		(self.from().min(len), self.to().min(len))
	}

	/// Returns the direction of this range.
	#[inline]
	pub fn direction(&self) -> Direction {
		if self.head < self.anchor { Direction::Backward } else { Direction::Forward }
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
		if self.direction() == direction { self } else { self.flip() }
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

	/// Returns true if the position is within the range (inclusive of max).
	///
	/// In the 1-cell minimum model, a range [min, max] selects characters
	/// from min to max inclusive.
	pub fn contains(&self, pos: CharIdx) -> bool {
		pos >= self.min() && pos <= self.max()
	}

	/// Returns true if this range overlaps with another.
	pub fn overlaps(&self, other: &Range) -> bool {
		self.min() <= other.max() && other.min() <= self.max()
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
