//! Abstract geometry types for layout.
//!
//! These types define rectangles, positions, and padding without depending on
//! any terminal or UI library. Conversion to tome_tui types happens at the UI
//! boundary via `From` trait implementations.

use serde::{Deserialize, Serialize};

/// A rectangle with position and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rect {
	pub x: u16,
	pub y: u16,
	pub width: u16,
	pub height: u16,
}

impl Rect {
	/// Creates a new rectangle.
	pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}

	/// Returns the area of the rectangle.
	pub const fn area(&self) -> u16 {
		self.width.saturating_mul(self.height)
	}

	/// Returns true if the rectangle has zero area.
	pub const fn is_empty(&self) -> bool {
		self.width == 0 || self.height == 0
	}

	/// Returns the left edge x coordinate.
	pub const fn left(&self) -> u16 {
		self.x
	}

	/// Returns the right edge x coordinate (exclusive).
	pub const fn right(&self) -> u16 {
		self.x.saturating_add(self.width)
	}

	/// Returns the top edge y coordinate.
	pub const fn top(&self) -> u16 {
		self.y
	}

	/// Returns the bottom edge y coordinate (exclusive).
	pub const fn bottom(&self) -> u16 {
		self.y.saturating_add(self.height)
	}

	/// Returns the intersection of two rectangles.
	pub fn intersection(&self, other: Self) -> Self {
		let x1 = self.x.max(other.x);
		let y1 = self.y.max(other.y);
		let x2 = self.right().min(other.right());
		let y2 = self.bottom().min(other.bottom());

		if x1 >= x2 || y1 >= y2 {
			Self::default()
		} else {
			Self::new(x1, y1, x2 - x1, y2 - y1)
		}
	}
}

/// A position (x, y coordinate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Position {
	pub x: u16,
	pub y: u16,
}

impl Position {
	/// Creates a new position.
	pub const fn new(x: u16, y: u16) -> Self {
		Self { x, y }
	}
}

/// Padding around content (top, right, bottom, left).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Padding {
	pub top: u16,
	pub right: u16,
	pub bottom: u16,
	pub left: u16,
}

impl Padding {
	/// Creates uniform padding on all sides.
	pub const fn uniform(value: u16) -> Self {
		Self {
			top: value,
			right: value,
			bottom: value,
			left: value,
		}
	}

	/// Creates horizontal padding (left and right).
	pub const fn horizontal(value: u16) -> Self {
		Self {
			top: 0,
			right: value,
			bottom: 0,
			left: value,
		}
	}

	/// Creates vertical padding (top and bottom).
	pub const fn vertical(value: u16) -> Self {
		Self {
			top: value,
			right: 0,
			bottom: value,
			left: 0,
		}
	}

	/// Creates padding with specific values for each side.
	pub const fn new(top: u16, right: u16, bottom: u16, left: u16) -> Self {
		Self {
			top,
			right,
			bottom,
			left,
		}
	}

	/// Returns the total horizontal padding.
	pub const fn horizontal_total(&self) -> u16 {
		self.left.saturating_add(self.right)
	}

	/// Returns the total vertical padding.
	pub const fn vertical_total(&self) -> u16 {
		self.top.saturating_add(self.bottom)
	}
}

/// Border style for widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BorderKind {
	/// Single-line border using ASCII characters.
	#[default]
	Plain,
	/// Rounded corners using Unicode box-drawing characters.
	Rounded,
	/// Double-line border.
	Double,
	/// Thick border using block characters.
	Thick,
	/// No visible border, just padding space.
	Padded,
}

// Conversion to tome_tui types

#[cfg(feature = "tome-tui")]
impl From<Rect> for tome_tui::layout::Rect {
	fn from(rect: Rect) -> Self {
		Self::new(rect.x, rect.y, rect.width, rect.height)
	}
}

#[cfg(feature = "tome-tui")]
impl From<tome_tui::layout::Rect> for Rect {
	fn from(rect: tome_tui::layout::Rect) -> Self {
		Self::new(rect.x, rect.y, rect.width, rect.height)
	}
}

#[cfg(feature = "tome-tui")]
impl From<Position> for tome_tui::layout::Position {
	fn from(pos: Position) -> Self {
		Self::new(pos.x, pos.y)
	}
}

#[cfg(feature = "tome-tui")]
impl From<tome_tui::layout::Position> for Position {
	fn from(pos: tome_tui::layout::Position) -> Self {
		Self::new(pos.x, pos.y)
	}
}

#[cfg(feature = "tome-tui")]
impl From<Padding> for tome_tui::widgets::block::Padding {
	fn from(padding: Padding) -> Self {
		Self::new(padding.left, padding.right, padding.top, padding.bottom)
	}
}

#[cfg(feature = "tome-tui")]
impl From<tome_tui::widgets::block::Padding> for Padding {
	fn from(padding: tome_tui::widgets::block::Padding) -> Self {
		Self::new(padding.top, padding.right, padding.bottom, padding.left)
	}
}

#[cfg(feature = "tome-tui")]
impl From<BorderKind> for tome_tui::widgets::BorderType {
	fn from(kind: BorderKind) -> Self {
		match kind {
			BorderKind::Plain => Self::Plain,
			BorderKind::Rounded => Self::Rounded,
			BorderKind::Double => Self::Double,
			BorderKind::Thick => Self::Thick,
			BorderKind::Padded => Self::Padded,
		}
	}
}

#[cfg(feature = "tome-tui")]
impl From<tome_tui::widgets::BorderType> for BorderKind {
	fn from(border_type: tome_tui::widgets::BorderType) -> Self {
		match border_type {
			tome_tui::widgets::BorderType::Plain => Self::Plain,
			tome_tui::widgets::BorderType::Rounded => Self::Rounded,
			tome_tui::widgets::BorderType::Double => Self::Double,
			tome_tui::widgets::BorderType::Thick => Self::Thick,
			tome_tui::widgets::BorderType::Padded => Self::Padded,
			// QuadrantInside/Outside don't have direct mappings
			_ => Self::Plain,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_rect_intersection() {
		let a = Rect::new(0, 0, 10, 10);
		let b = Rect::new(5, 5, 10, 10);
		let intersection = a.intersection(b);
		assert_eq!(intersection, Rect::new(5, 5, 5, 5));
	}

	#[test]
	fn test_rect_no_intersection() {
		let a = Rect::new(0, 0, 5, 5);
		let b = Rect::new(10, 10, 5, 5);
		let intersection = a.intersection(b);
		assert!(intersection.is_empty());
	}

	#[test]
	fn test_padding_totals() {
		let padding = Padding::new(1, 2, 3, 4);
		assert_eq!(padding.horizontal_total(), 6);
		assert_eq!(padding.vertical_total(), 4);
	}

	#[cfg(feature = "tome-tui")]
	#[test]
	fn test_rect_roundtrip() {
		let rect = Rect::new(10, 20, 100, 50);
		let tome_tui_rect: tome_tui::layout::Rect = rect.into();
		let back: Rect = tome_tui_rect.into();
		assert_eq!(rect, back);
	}
}
