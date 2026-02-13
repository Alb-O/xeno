#![warn(missing_docs)]
//! Backend-agnostic geometry primitives used by editor core subsystems.
//!
//! These types are intentionally small and stable so layout/state code can
//! remain independent of frontend widget/back-end crates.

use core::cmp::{max, min};
use core::fmt;

/// A rectangle in screen-space coordinates.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Rect {
	/// The x coordinate of the top-left corner.
	pub x: u16,
	/// The y coordinate of the top-left corner.
	pub y: u16,
	/// The rectangle width.
	pub width: u16,
	/// The rectangle height.
	pub height: u16,
}

impl Rect {
	/// Zero-sized rectangle at `(0, 0)`.
	pub const ZERO: Self = Self {
		x: 0,
		y: 0,
		width: 0,
		height: 0,
	};

	/// The minimum representable rectangle.
	pub const MIN: Self = Self::ZERO;

	/// The maximum representable rectangle.
	pub const MAX: Self = Self::new(0, 0, u16::MAX, u16::MAX);

	/// Creates a rectangle while clamping `width`/`height` to keep bounds in `u16`.
	pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
		let width = x.saturating_add(width) - x;
		let height = y.saturating_add(height) - y;
		Self { x, y, width, height }
	}

	/// Returns the rectangle area in cells.
	pub const fn area(self) -> u32 {
		(self.width as u32) * (self.height as u32)
	}

	/// Returns whether the rectangle has zero area.
	pub const fn is_empty(self) -> bool {
		self.width == 0 || self.height == 0
	}

	/// Left edge coordinate.
	pub const fn left(self) -> u16 {
		self.x
	}

	/// Right edge coordinate (exclusive).
	pub const fn right(self) -> u16 {
		self.x.saturating_add(self.width)
	}

	/// Top edge coordinate.
	pub const fn top(self) -> u16 {
		self.y
	}

	/// Bottom edge coordinate (exclusive).
	pub const fn bottom(self) -> u16 {
		self.y.saturating_add(self.height)
	}

	/// Returns true if `position` lies inside this rectangle.
	pub const fn contains(self, position: Position) -> bool {
		position.x >= self.x && position.x < self.right() && position.y >= self.y && position.y < self.bottom()
	}

	/// Returns the smallest rectangle that contains both rectangles.
	#[must_use]
	pub fn union(self, other: Self) -> Self {
		let x1 = min(self.x, other.x);
		let y1 = min(self.y, other.y);
		let x2 = max(self.right(), other.right());
		let y2 = max(self.bottom(), other.bottom());
		Self::new(x1, y1, x2.saturating_sub(x1), y2.saturating_sub(y1))
	}

	/// Returns the overlapping rectangle, or zero-sized when disjoint.
	#[must_use]
	pub fn intersection(self, other: Self) -> Self {
		let x1 = max(self.x, other.x);
		let y1 = max(self.y, other.y);
		let x2 = min(self.right(), other.right());
		let y2 = min(self.bottom(), other.bottom());
		Self::new(x1, y1, x2.saturating_sub(x1), y2.saturating_sub(y1))
	}

	/// Returns whether the rectangles overlap.
	pub const fn intersects(self, other: Self) -> bool {
		self.x < other.right() && self.right() > other.x && self.y < other.bottom() && self.bottom() > other.y
	}

	/// Returns a rectangle with identical origin and clamped size/position inside `other`.
	#[must_use]
	pub fn clamp(self, other: Self) -> Self {
		let width = self.width.min(other.width);
		let height = self.height.min(other.height);
		let x = self.x.clamp(other.x, other.right().saturating_sub(width));
		let y = self.y.clamp(other.y, other.bottom().saturating_sub(height));
		Self::new(x, y, width, height)
	}

	/// Returns the top-left corner as a [`Position`].
	pub const fn as_position(self) -> Position {
		Position { x: self.x, y: self.y }
	}
}

impl fmt::Display for Rect {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}x{}+{}+{}", self.width, self.height, self.x, self.y)
	}
}

/// A point in screen-space coordinates.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct Position {
	/// X coordinate.
	pub x: u16,
	/// Y coordinate.
	pub y: u16,
}

impl Position {
	/// Origin position `(0, 0)`.
	pub const ORIGIN: Self = Self::new(0, 0);

	/// Minimum representable position.
	pub const MIN: Self = Self::ORIGIN;

	/// Maximum representable position.
	pub const MAX: Self = Self::new(u16::MAX, u16::MAX);

	/// Creates a position.
	pub const fn new(x: u16, y: u16) -> Self {
		Self { x, y }
	}
}

impl From<(u16, u16)> for Position {
	fn from((x, y): (u16, u16)) -> Self {
		Self { x, y }
	}
}

impl From<Position> for (u16, u16) {
	fn from(position: Position) -> Self {
		(position.x, position.y)
	}
}

impl From<Rect> for Position {
	fn from(rect: Rect) -> Self {
		rect.as_position()
	}
}

impl fmt::Display for Position {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "({}, {})", self.x, self.y)
	}
}

#[cfg(feature = "tui-style")]
impl From<xeno_tui::layout::Rect> for Rect {
	fn from(value: xeno_tui::layout::Rect) -> Self {
		Self {
			x: value.x,
			y: value.y,
			width: value.width,
			height: value.height,
		}
	}
}

#[cfg(feature = "tui-style")]
impl From<Rect> for xeno_tui::layout::Rect {
	fn from(value: Rect) -> Self {
		Self::new(value.x, value.y, value.width, value.height)
	}
}

#[cfg(feature = "tui-style")]
impl From<xeno_tui::layout::Position> for Position {
	fn from(value: xeno_tui::layout::Position) -> Self {
		Self { x: value.x, y: value.y }
	}
}

#[cfg(feature = "tui-style")]
impl From<Position> for xeno_tui::layout::Position {
	fn from(value: Position) -> Self {
		Self::new(value.x, value.y)
	}
}

#[cfg(test)]
mod tests {
	use super::{Position, Rect};

	#[test]
	fn new_rect_saturates_dimensions() {
		let rect = Rect::new(u16::MAX - 1, u16::MAX - 1, 10, 10);
		assert_eq!(rect.width, 1);
		assert_eq!(rect.height, 1);
	}

	#[test]
	fn rect_edges_are_exclusive() {
		let rect = Rect::new(10, 5, 3, 2);
		assert_eq!(rect.left(), 10);
		assert_eq!(rect.right(), 13);
		assert_eq!(rect.top(), 5);
		assert_eq!(rect.bottom(), 7);
	}

	#[test]
	fn contains_uses_inclusive_origin_exclusive_max() {
		let rect = Rect::new(10, 5, 3, 2);
		assert!(rect.contains(Position::new(10, 5)));
		assert!(rect.contains(Position::new(12, 6)));
		assert!(!rect.contains(Position::new(13, 6)));
		assert!(!rect.contains(Position::new(12, 7)));
	}
}
