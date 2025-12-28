#![warn(missing_docs)]
use core::array::TryFromSliceError;
use core::cmp::{max, min};
use core::fmt;

pub use self::iter::{Columns, Positions, Rows};
use crate::layout::{Margin, Offset, Position, Size};

mod iter;
mod ops;

use super::{Constraint, Flex, Layout};

/// A rectangular area in the terminal.
///
/// A `Rect` represents a rectangular region in the terminal coordinate system, defined by its
/// top-left corner position and dimensions. This is the fundamental building block for all layout
/// operations and widget rendering in Ratatui.
///
/// Rectangles are used throughout the layout system to define areas where widgets can be rendered.
/// They are typically created by [`Layout`] operations that divide terminal space, but can also be
/// manually constructed for specific positioning needs.
///
/// The coordinate system uses the top-left corner as the origin (0, 0), with x increasing to the
/// right and y increasing downward. All measurements are in character cells.
///
/// # Construction and Conversion
///
/// - [`new`](Self::new) - Create a new rectangle from coordinates and dimensions
/// - [`as_position`](Self::as_position) - Convert to a position at the top-left corner
/// - [`as_size`](Self::as_size) - Convert to a size representing the dimensions
/// - [`from((Position, Size))`](Self::from) - Create from `(Position, Size)` tuple
/// - [`from(((u16, u16), (u16, u16)))`](Self::from) - Create from `((u16, u16), (u16, u16))`
///   coordinate and dimension tuples
/// - [`into((Position, Size))`] - Convert to `(Position, Size)` tuple
/// - [`default`](Self::default) - Create a zero-sized rectangle at origin
///
/// # Geometry and Properties
///
/// - [`area`](Self::area) - Calculate the total area in character cells
/// - [`is_empty`](Self::is_empty) - Check if the rectangle has zero area
/// - [`left`](Self::left), [`right`](Self::right), [`top`](Self::top), [`bottom`](Self::bottom) -
///   Get edge coordinates
///
/// # Spatial Operations
///
/// - [`inner`](Self::inner), [`outer`](Self::outer) - Apply margins to shrink or expand
/// - [`offset`](Self::offset) - Move the rectangle by a relative amount
/// - [`resize`](Self::resize) - Change the rectangle size while keeping the bottom/right in range
/// - [`union`](Self::union) - Combine with another rectangle to create a bounding box
/// - [`intersection`](Self::intersection) - Find the overlapping area with another rectangle
/// - [`clamp`](Self::clamp) - Constrain the rectangle to fit within another
///
/// # Positioning and Centering
///
/// - [`centered_horizontally`](Self::centered_horizontally) - Center horizontally within a
///   constraint
/// - [`centered_vertically`](Self::centered_vertically) - Center vertically within a constraint
/// - [`centered`](Self::centered) - Center both horizontally and vertically
///
/// # Testing and Iteration
///
/// - [`contains`](Self::contains) - Check if a position is within the rectangle
/// - [`intersects`](Self::intersects) - Check if it overlaps with another rectangle
/// - [`rows`](Self::rows) - Iterate over horizontal rows within the rectangle
/// - [`columns`](Self::columns) - Iterate over vertical columns within the rectangle
/// - [`positions`](Self::positions) - Iterate over all positions within the rectangle
///
/// # Examples
///
/// To create a new `Rect`, use [`Rect::new`]. The size of the `Rect` will be clamped to keep the
/// right and bottom coordinates within `u16`. Note that this clamping does not occur when creating
/// a `Rect` directly.
///
/// ```rust
/// use tome_tui::layout::Rect;
///
/// let rect = Rect::new(1, 2, 3, 4);
/// assert_eq!(
///     rect,
///     Rect {
///         x: 1,
///         y: 2,
///         width: 3,
///         height: 4
///     }
/// );
/// ```
///
/// You can also create a `Rect` from a [`Position`] and a [`Size`].
///
/// ```rust
/// use tome_tui::layout::{Position, Rect, Size};
///
/// let position = Position::new(1, 2);
/// let size = Size::new(3, 4);
/// let rect = Rect::from((position, size));
/// assert_eq!(
///     rect,
///     Rect {
///         x: 1,
///         y: 2,
///         width: 3,
///         height: 4
///     }
/// );
/// ```
///
/// To move a `Rect` without modifying its size, add or subtract an [`Offset`] to it.
///
/// ```rust
/// use tome_tui::layout::{Offset, Rect};
///
/// let rect = Rect::new(1, 2, 3, 4);
/// let offset = Offset::new(5, 6);
/// let moved_rect = rect + offset;
/// assert_eq!(moved_rect, Rect::new(6, 8, 3, 4));
/// ```
///
/// To resize a `Rect` while ensuring it stays within bounds, use [`Rect::resize`]. The size is
/// clamped so that `right()` and `bottom()` do not exceed `u16::MAX`.
///
/// ```rust
/// use tome_tui::layout::{Rect, Size};
///
/// let rect = Rect::new(u16::MAX - 1, u16::MAX - 1, 1, 1).resize(Size::new(10, 10));
/// assert_eq!(rect, Rect::new(u16::MAX - 1, u16::MAX - 1, 1, 1));
/// ```
///
/// For comprehensive layout documentation and examples, see the [`layout`](crate::layout) module.

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect {
	/// The x coordinate of the top left corner of the `Rect`.
	pub x: u16,
	/// The y coordinate of the top left corner of the `Rect`.
	pub y: u16,
	/// The width of the `Rect`.
	pub width: u16,
	/// The height of the `Rect`.
	pub height: u16,
}

impl fmt::Display for Rect {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}x{}+{}+{}", self.width, self.height, self.x, self.y)
	}
}

impl Rect {
	/// A zero sized Rect at position 0,0
	pub const ZERO: Self = Self {
		x: 0,
		y: 0,
		width: 0,
		height: 0,
	};

	/// The minimum possible Rect
	pub const MIN: Self = Self::ZERO;

	/// The maximum possible Rect
	pub const MAX: Self = Self::new(0, 0, u16::MAX, u16::MAX);

	/// Creates a new `Rect`, with width and height limited to keep both bounds within `u16`.
	///
	/// If the width or height would cause the right or bottom coordinate to be larger than the
	/// maximum value of `u16`, the width or height will be clamped to keep the right or bottom
	/// coordinate within `u16`.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::Rect;
	///
	/// let rect = Rect::new(1, 2, 3, 4);
	/// ```
	pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
		let width = x.saturating_add(width) - x;
		let height = y.saturating_add(height) - y;
		Self {
			x,
			y,
			width,
			height,
		}
	}

	/// The area of the `Rect`.
	pub const fn area(self) -> u32 {
		(self.width as u32) * (self.height as u32)
	}

	/// Returns true if the `Rect` has no area.
	pub const fn is_empty(self) -> bool {
		self.width == 0 || self.height == 0
	}

	/// Returns the left coordinate of the `Rect`.
	pub const fn left(self) -> u16 {
		self.x
	}

	/// Returns the right coordinate of the `Rect`. This is the first coordinate outside of the
	/// `Rect`.
	///
	/// If the right coordinate is larger than the maximum value of u16, it will be clamped to
	/// `u16::MAX`.
	pub const fn right(self) -> u16 {
		self.x.saturating_add(self.width)
	}

	/// Returns the top coordinate of the `Rect`.
	pub const fn top(self) -> u16 {
		self.y
	}

	/// Returns the bottom coordinate of the `Rect`. This is the first coordinate outside of the
	/// `Rect`.
	///
	/// If the bottom coordinate is larger than the maximum value of u16, it will be clamped to
	/// `u16::MAX`.
	pub const fn bottom(self) -> u16 {
		self.y.saturating_add(self.height)
	}

	/// Returns a new `Rect` inside the current one, with the given margin on each side.
	///
	/// If the margin is larger than the `Rect`, the returned `Rect` will have no area.
	#[must_use = "method returns the modified value"]
	pub const fn inner(self, margin: Margin) -> Self {
		let doubled_margin_horizontal = margin.horizontal.saturating_mul(2);
		let doubled_margin_vertical = margin.vertical.saturating_mul(2);

		if self.width < doubled_margin_horizontal || self.height < doubled_margin_vertical {
			Self::ZERO
		} else {
			Self {
				x: self.x.saturating_add(margin.horizontal),
				y: self.y.saturating_add(margin.vertical),
				width: self.width.saturating_sub(doubled_margin_horizontal),
				height: self.height.saturating_sub(doubled_margin_vertical),
			}
		}
	}

	/// Returns a new `Rect` outside the current one, with the given margin applied on each side.
	///
	/// If the margin causes the `Rect`'s bounds to be outside the range of a `u16`, the `Rect` will
	/// be truncated to keep the bounds within `u16`. This will cause the size of the `Rect` to
	/// change.
	///
	/// The generated `Rect` may not fit inside the buffer or containing area, so it consider
	/// constraining the resulting `Rect` with [`Rect::clamp`] before using it.
	#[must_use = "method returns the modified value"]
	pub const fn outer(self, margin: Margin) -> Self {
		let x = self.x.saturating_sub(margin.horizontal);
		let y = self.y.saturating_sub(margin.vertical);
		let width = self
			.right()
			.saturating_add(margin.horizontal)
			.saturating_sub(x);
		let height = self
			.bottom()
			.saturating_add(margin.vertical)
			.saturating_sub(y);
		Self {
			x,
			y,
			width,
			height,
		}
	}

	/// Moves the `Rect` without modifying its size.
	///
	/// Moves the `Rect` according to the given offset without modifying its [`width`](Rect::width)
	/// or [`height`](Rect::height).
	/// - Positive `x` moves the whole `Rect` to the right, negative to the left.
	/// - Positive `y` moves the whole `Rect` to the bottom, negative to the top.
	///
	/// See [`Offset`] for details.
	#[must_use = "method returns the modified value"]
	pub fn offset(self, offset: Offset) -> Self {
		self + offset
	}

	/// Resizes the `Rect`, clamping to keep the right and bottom within `u16::MAX`.
	///
	/// The position is preserved. If the requested size would push the `Rect` beyond the bounds of
	/// `u16`, the width or height is reduced so that [`right`](Self::right) and
	/// [`bottom`](Self::bottom) remain within range.
	#[must_use = "method returns the modified value"]
	pub const fn resize(self, size: Size) -> Self {
		Self {
			width: self.x.saturating_add(size.width).saturating_sub(self.x),
			height: self.y.saturating_add(size.height).saturating_sub(self.y),
			..self
		}
	}

	/// Returns a new `Rect` that contains both the current one and the given one.
	#[must_use = "method returns the modified value"]
	pub fn union(self, other: Self) -> Self {
		let x1 = min(self.x, other.x);
		let y1 = min(self.y, other.y);
		let x2 = max(self.right(), other.right());
		let y2 = max(self.bottom(), other.bottom());
		Self {
			x: x1,
			y: y1,
			width: x2.saturating_sub(x1),
			height: y2.saturating_sub(y1),
		}
	}

	/// Returns a new `Rect` that is the intersection of the current one and the given one.
	///
	/// If the two `Rect`s do not intersect, the returned `Rect` will have no area.
	#[must_use = "method returns the modified value"]
	pub fn intersection(self, other: Self) -> Self {
		let x1 = max(self.x, other.x);
		let y1 = max(self.y, other.y);
		let x2 = min(self.right(), other.right());
		let y2 = min(self.bottom(), other.bottom());
		Self {
			x: x1,
			y: y1,
			width: x2.saturating_sub(x1),
			height: y2.saturating_sub(y1),
		}
	}

	/// Returns true if the two `Rect`s intersect.
	pub const fn intersects(self, other: Self) -> bool {
		self.x < other.right()
			&& self.right() > other.x
			&& self.y < other.bottom()
			&& self.bottom() > other.y
	}

	/// Returns true if the given position is inside the `Rect`.
	///
	/// The position is considered inside the `Rect` if it is on the `Rect`'s border.
	///
	/// # Examples
	///
	/// ```rust
	/// use tome_tui::layout::{Position, Rect};
	///
	/// let rect = Rect::new(1, 2, 3, 4);
	/// assert!(rect.contains(Position { x: 1, y: 2 }));
	/// ````
	pub const fn contains(self, position: Position) -> bool {
		position.x >= self.x
			&& position.x < self.right()
			&& position.y >= self.y
			&& position.y < self.bottom()
	}

	/// Clamp this `Rect` to fit inside the other `Rect`.
	///
	/// If the width or height of this `Rect` is larger than the other `Rect`, it will be clamped to
	/// the other `Rect`'s width or height.
	///
	/// If the left or top coordinate of this `Rect` is smaller than the other `Rect`, it will be
	/// clamped to the other `Rect`'s left or top coordinate.
	///
	/// If the right or bottom coordinate of this `Rect` is larger than the other `Rect`, it will be
	/// clamped to the other `Rect`'s right or bottom coordinate.
	///
	/// This is different from [`Rect::intersection`] because it will move this `Rect` to fit inside
	/// the other `Rect`, while [`Rect::intersection`] instead would keep this `Rect`'s position and
	/// truncate its size to only that which is inside the other `Rect`.
	///
	/// # Examples
	///
	/// ```rust
	/// use tome_tui::layout::Rect;
	///
	/// let area = Rect::new(0, 0, 100, 100);
	/// let rect = Rect::new(80, 80, 30, 30).clamp(area);
	/// assert_eq!(rect, Rect::new(70, 70, 30, 30));
	/// ```
	#[must_use = "method returns the modified value"]
	pub fn clamp(self, other: Self) -> Self {
		let width = self.width.min(other.width);
		let height = self.height.min(other.height);
		let x = self.x.clamp(other.x, other.right().saturating_sub(width));
		let y = self.y.clamp(other.y, other.bottom().saturating_sub(height));
		Self::new(x, y, width, height)
	}

	/// An iterator over rows within the `Rect`.
	///
	/// Each row is a full `Rect` region with height 1 that can be used for rendering widgets
	/// or as input to further layout methods.
	///
	/// # Example
	///
	/// ```
	/// use tome_tui::buffer::Buffer;
	/// use tome_tui::layout::{Constraint, Layout, Rect};
	/// use tome_tui::widgets::Widget;
	///
	/// fn render_list(area: Rect, buf: &mut Buffer) {
	///     // Renders "Item 0", "Item 1", etc. in each row
	///     for (i, row) in area.rows().enumerate() {
	///         format!("Item {i}").render(row, buf);
	///     }
	/// }
	///
	/// fn render_with_nested_layout(area: Rect, buf: &mut Buffer) {
	///     // Splits each row into left/right areas and renders labels and content
	///     for (i, row) in area.rows().take(3).enumerate() {
	///         let [left, right] =
	///             Layout::horizontal([Constraint::Percentage(30), Constraint::Fill(1)]).areas(row);
	///
	///         format!("{i}:").render(left, buf);
	///         "Content".render(right, buf);
	///     }
	/// }
	/// ```
	pub const fn rows(self) -> Rows {
		Rows::new(self)
	}

	/// An iterator over columns within the `Rect`.
	///
	/// Each column is a full `Rect` region with width 1 that can be used for rendering widgets
	/// or as input to further layout methods.
	///
	/// # Example
	///
	/// ```
	/// use tome_tui::buffer::Buffer;
	/// use tome_tui::layout::Rect;
	/// use tome_tui::widgets::Widget;
	///
	/// fn render_columns(area: Rect, buf: &mut Buffer) {
	///     // Renders column indices (0-9 repeating) in each column
	///     for (i, column) in area.columns().enumerate() {
	///         format!("{}", i % 10).render(column, buf);
	///     }
	/// }
	/// ```
	pub const fn columns(self) -> Columns {
		Columns::new(self)
	}

	/// An iterator over the positions within the `Rect`.
	///
	/// The positions are returned in a row-major order (left-to-right, top-to-bottom).
	/// Each position is a `Position` that represents a single cell coordinate.
	///
	/// # Example
	///
	/// ```
	/// use tome_tui::buffer::Buffer;
	/// use tome_tui::layout::{Position, Rect};
	/// use tome_tui::widgets::Widget;
	///
	/// fn render_positions(area: Rect, buf: &mut Buffer) {
	///     // Renders position indices (0-9 repeating) at each cell position
	///     for (i, position) in area.positions().enumerate() {
	///         buf[position].set_symbol(&format!("{}", i % 10));
	///     }
	/// }
	/// ```
	pub const fn positions(self) -> Positions {
		Positions::new(self)
	}

	/// Returns a [`Position`] with the same coordinates as this `Rect`.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::Rect;
	///
	/// let rect = Rect::new(1, 2, 3, 4);
	/// let position = rect.as_position();
	/// ````
	pub const fn as_position(self) -> Position {
		Position {
			x: self.x,
			y: self.y,
		}
	}

	/// Converts the `Rect` into a size struct.
	pub const fn as_size(self) -> Size {
		Size {
			width: self.width,
			height: self.height,
		}
	}

	/// Returns a new Rect, centered horizontally based on the provided constraint.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::Constraint;
	/// use tome_tui::terminal::Frame;
	///
	/// fn render(frame: &mut Frame) {
	///     let area = frame.area().centered_horizontally(Constraint::Ratio(1, 2));
	/// }
	/// ```
	#[must_use]
	pub fn centered_horizontally(self, constraint: Constraint) -> Self {
		let [area] = self.layout(&Layout::horizontal([constraint]).flex(Flex::Center));
		area
	}

	/// Returns a new Rect, centered vertically based on the provided constraint.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::Constraint;
	/// use tome_tui::terminal::Frame;
	///
	/// fn render(frame: &mut Frame) {
	///     let area = frame.area().centered_vertically(Constraint::Ratio(1, 2));
	/// }
	/// ```
	#[must_use]
	pub fn centered_vertically(self, constraint: Constraint) -> Self {
		let [area] = self.layout(&Layout::vertical([constraint]).flex(Flex::Center));
		area
	}

	/// Returns a new Rect, centered horizontally and vertically based on the provided constraints.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::Constraint;
	/// use tome_tui::terminal::Frame;
	///
	/// fn render(frame: &mut Frame) {
	///     let area = frame
	///         .area()
	///         .centered(Constraint::Ratio(1, 2), Constraint::Ratio(1, 3));
	/// }
	/// ```
	#[must_use]
	pub fn centered(
		self,
		horizontal_constraint: Constraint,
		vertical_constraint: Constraint,
	) -> Self {
		self.centered_horizontally(horizontal_constraint)
			.centered_vertically(vertical_constraint)
	}

	/// Split the rect into a number of sub-rects according to the given [`Layout`].
	///
	/// An ergonomic wrapper around [`Layout::split`] that returns an array of `Rect`s instead of
	/// `Rc<[Rect]>`.
	///
	/// This method requires the number of constraints to be known at compile time. If you don't
	/// know the number of constraints at compile time, use [`Layout::split`] instead.
	///
	/// # Panics
	///
	/// Panics if the number of constraints is not equal to the length of the returned array.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::{Constraint, Layout, Rect};
	///
	/// let area = Rect::new(0, 0, 10, 10);
	/// let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
	/// let [top, main] = area.layout(&layout);
	/// assert_eq!(top, Rect::new(0, 0, 10, 1));
	/// assert_eq!(main, Rect::new(0, 1, 10, 9));
	///
	/// // or explicitly specify the number of constraints:
	/// let areas = area.layout::<2>(&layout);
	/// assert_eq!(areas, [Rect::new(0, 0, 10, 1), Rect::new(0, 1, 10, 9),]);
	/// ```
	#[must_use]
	pub fn layout<const N: usize>(self, layout: &Layout) -> [Self; N] {
		let areas = layout.split(self);
		areas.as_ref().try_into().unwrap_or_else(|_| {
			panic!(
				"invalid number of rects: expected {N}, found {}",
				areas.len()
			)
		})
	}

	/// Split the rect into a number of sub-rects according to the given [`Layout`].
	///
	/// An ergonomic wrapper around [`Layout::split`] that returns a [`Vec`] of `Rect`s instead of
	/// `Rc<[Rect]>`.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::{Constraint, Layout, Rect};
	///
	/// let area = Rect::new(0, 0, 10, 10);
	/// let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
	/// let areas = area.layout_vec(&layout);
	/// assert_eq!(areas, vec![Rect::new(0, 0, 10, 1), Rect::new(0, 1, 10, 9),]);
	/// ```
	///
	/// [`Vec`]: alloc::vec::Vec
	#[must_use]
	pub fn layout_vec(self, layout: &Layout) -> alloc::vec::Vec<Self> {
		layout.split(self).as_ref().to_vec()
	}

	/// Try to split the rect into a number of sub-rects according to the given [`Layout`].
	///
	/// An ergonomic wrapper around [`Layout::split`] that returns an array of `Rect`s instead of
	/// `Rc<[Rect]>`.
	///
	/// # Errors
	///
	/// Returns an error if the number of constraints is not equal to the length of the returned
	/// array.
	///
	/// # Examples
	///
	/// ```
	/// use tome_tui::layout::{Constraint, Layout, Rect};
	///
	/// let area = Rect::new(0, 0, 10, 10);
	/// let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
	/// let [top, main] = area.try_layout(&layout)?;
	/// assert_eq!(top, Rect::new(0, 0, 10, 1));
	/// assert_eq!(main, Rect::new(0, 1, 10, 9));
	///
	/// // or explicitly specify the number of constraints:
	/// let areas = area.try_layout::<2>(&layout)?;
	/// assert_eq!(areas, [Rect::new(0, 0, 10, 1), Rect::new(0, 1, 10, 9),]);
	/// # Ok::<(), core::array::TryFromSliceError>(())
	/// ``````
	pub fn try_layout<const N: usize>(
		self,
		layout: &Layout,
	) -> Result<[Self; N], TryFromSliceError> {
		layout.split(self).as_ref().try_into()
	}

	/// indents the x value of the `Rect` by a given `offset`
	///
	/// This is pub(crate) for now as we need to stabilize the naming / design of this API.
	#[must_use]
	pub(crate) const fn indent_x(self, offset: u16) -> Self {
		Self {
			x: self.x.saturating_add(offset),
			width: self.width.saturating_sub(offset),
			..self
		}
	}
}

impl From<(Position, Size)> for Rect {
	fn from((position, size): (Position, Size)) -> Self {
		Self {
			x: position.x,
			y: position.y,
			width: size.width,
			height: size.height,
		}
	}
}

impl From<Size> for Rect {
	/// Creates a new `Rect` with the given size at [`Position::ORIGIN`] (0, 0).
	fn from(size: Size) -> Self {
		Self {
			x: 0,
			y: 0,
			width: size.width,
			height: size.height,
		}
	}
}

#[cfg(test)]
mod tests;
