#![warn(missing_docs)]
//! A module for the [`Buffer`] and [`Cell`] types.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Index, IndexMut};
use core::{cmp, fmt};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::layout::{Position, Rect};
use crate::style::Style;
use crate::text::{Line, Span};

mod assert;

mod cell;
pub use cell::Cell;

/// Intermediate buffer for widget rendering.
///
/// A grid of [`Cell`]s (grapheme + fg/bg colors) that widgets draw to before terminal output.
/// Index via `buf[(x, y)]` or `buf[Position]`; use [`Self::cell`]/[`Self::cell_mut`] for safe access.
#[derive(Default, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Buffer {
	/// The area represented by this buffer
	pub area: Rect,
	/// The content of the buffer. The length of this Vec should always be equal to area.width *
	/// area.height
	pub content: Vec<Cell>,
}

impl Buffer {
	/// Returns a Buffer with all cells set to the default one
	#[must_use]
	pub fn empty(area: Rect) -> Self {
		Self::filled(area, Cell::EMPTY)
	}

	/// Returns a Buffer with all cells initialized with the attributes of the given Cell
	#[must_use]
	pub fn filled(area: Rect, cell: Cell) -> Self {
		let size = area.area() as usize;
		let content = vec![cell; size];
		Self { area, content }
	}

	/// Returns a Buffer containing the given lines
	#[must_use]
	pub fn with_lines<'a, Iter>(lines: Iter) -> Self
	where
		Iter: IntoIterator,
		Iter::Item: Into<Line<'a>>,
	{
		let lines = lines.into_iter().map(Into::into).collect::<Vec<_>>();
		let height = lines.len() as u16;
		let width = lines.iter().map(Line::width).max().unwrap_or_default() as u16;
		let mut buffer = Self::empty(Rect::new(0, 0, width, height));
		for (y, line) in lines.iter().enumerate() {
			buffer.set_line(0, y as u16, line, width);
		}
		buffer
	}

	/// Returns the content of the buffer as a slice
	pub fn content(&self) -> &[Cell] {
		&self.content
	}

	/// Returns the area covered by this buffer
	pub const fn area(&self) -> &Rect {
		&self.area
	}

	/// Returns a reference to the [`Cell`] at the given coordinates
	///
	/// Callers should use [`Buffer[]`](Self::index) or [`Buffer::cell`] instead of this method.
	///
	/// Note: idiomatically methods named `get` usually return `Option<&T>`, but this method panics
	/// instead. This is kept for backwards compatibility. See [`cell`](Self::cell) for a safe
	/// alternative.
	///
	/// # Panics
	///
	/// Panics if the index is out of bounds.
	#[track_caller]
	#[deprecated = "use `Buffer[(x, y)]` instead. To avoid panicking, use `Buffer::cell((x, y))`. Both methods take `impl Into<Position>`."]
	#[must_use]
	pub fn get(&self, x: u16, y: u16) -> &Cell {
		let i = self.index_of(x, y);
		&self.content[i]
	}

	/// Returns a mutable reference to the [`Cell`] at the given coordinates.
	///
	/// Callers should use [`Buffer[]`](Self::index_mut) or [`Buffer::cell_mut`] instead of this
	/// method.
	///
	/// Note: idiomatically methods named `get_mut` usually return `Option<&mut T>`, but this method
	/// panics instead. This is kept for backwards compatibility. See [`cell_mut`](Self::cell_mut)
	/// for a safe alternative.
	///
	/// # Panics
	///
	/// Panics if the position is outside the `Buffer`'s area.
	#[track_caller]
	#[deprecated = "use `Buffer[(x, y)]` instead. To avoid panicking, use `Buffer::cell_mut((x, y))`. Both methods take `impl Into<Position>`."]
	#[must_use]
	pub fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
		let i = self.index_of(x, y);
		&mut self.content[i]
	}

	/// Returns a reference to the [`Cell`] at the given position or [`None`] if the position is
	/// outside the `Buffer`'s area.
	///
	/// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
	/// `Position::new(x, y)`).
	///
	/// For a method that panics when the position is outside the buffer instead of returning
	/// `None`, use [`Buffer[]`](Self::index).
	///
	/// # Examples
	///
	/// ```rust
	/// use crate::buffer::{Buffer, Cell};
	/// use crate::layout::{Position, Rect};
	///
	/// let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
	///
	/// assert_eq!(buffer.cell(Position::new(0, 0)), Some(&Cell::default()));
	/// assert_eq!(buffer.cell(Position::new(10, 10)), None);
	/// assert_eq!(buffer.cell((0, 0)), Some(&Cell::default()));
	/// assert_eq!(buffer.cell((10, 10)), None);
	/// ```
	#[must_use]
	pub fn cell<P: Into<Position>>(&self, position: P) -> Option<&Cell> {
		let position = position.into();
		let index = self.index_of_opt(position)?;
		self.content.get(index)
	}

	/// Returns a mutable reference to the [`Cell`] at the given position or [`None`] if the
	/// position is outside the `Buffer`'s area.
	///
	/// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
	/// `Position::new(x, y)`).
	///
	/// For a method that panics when the position is outside the buffer instead of returning
	/// `None`, use [`Buffer[]`](Self::index_mut).
	///
	/// # Examples
	///
	/// ```rust
	/// use crate::buffer::{Buffer, Cell};
	/// use crate::layout::{Position, Rect};
	/// use crate::style::{Color, Style};
	/// let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
	///
	/// if let Some(cell) = buffer.cell_mut(Position::new(0, 0)) {
	///     cell.set_symbol("A");
	/// }
	/// if let Some(cell) = buffer.cell_mut((0, 0)) {
	///     cell.set_style(Style::default().fg(Color::Red));
	/// }
	/// ```
	#[must_use]
	pub fn cell_mut<P: Into<Position>>(&mut self, position: P) -> Option<&mut Cell> {
		let position = position.into();
		let index = self.index_of_opt(position)?;
		self.content.get_mut(index)
	}

	/// Returns the index in the `Vec<Cell>` for the given global (x, y) coordinates.
	///
	/// Global coordinates are offset by the Buffer's area offset (`x`/`y`).
	///
	/// Usage discouraged, as it exposes `self.content` as a linearly indexable array, which limits
	/// potential future abstractions.
	///
	/// # Examples
	///
	/// ```
	/// use crate::buffer::Buffer;
	/// use crate::layout::Rect;
	///
	/// let buffer = Buffer::empty(Rect::new(200, 100, 10, 10));
	/// // Global coordinates to the top corner of this buffer's area
	/// assert_eq!(buffer.index_of(200, 100), 0);
	/// ```
	///
	/// # Panics
	///
	/// Panics when given an coordinate that is outside of this Buffer's area.
	///
	/// ```should_panic
	/// use crate::buffer::Buffer;
	/// use crate::layout::Rect;
	///
	/// let buffer = Buffer::empty(Rect::new(200, 100, 10, 10));
	/// // Top coordinate is outside of the buffer in global coordinate space, as the Buffer's area
	/// // starts at (200, 100).
	/// buffer.index_of(0, 0); // Panics
	/// ```
	#[track_caller]
	#[must_use]
	pub fn index_of(&self, x: u16, y: u16) -> usize {
		self.index_of_opt(Position { x, y }).unwrap_or_else(|| {
			panic!(
				"index outside of buffer: the area is {area:?} but index is ({x}, {y})",
				area = self.area,
			)
		})
	}

	/// Returns the index in the `Vec<Cell>` for the given global (x, y) coordinates.
	///
	/// Returns `None` if the given coordinates are outside of the Buffer's area.
	///
	/// Note that this is private to limit exposure of internal buffer layout.
	#[must_use]
	const fn index_of_opt(&self, position: Position) -> Option<usize> {
		let area = self.area;
		if !area.contains(position) {
			return None;
		}
		// remove offset
		let y = (position.y - self.area.y) as usize;
		let x = (position.x - self.area.x) as usize;
		let width = self.area.width as usize;
		Some(y * width + x)
	}

	/// Returns the (global) coordinates of a cell given its index.
	///
	/// Global coordinates are offset by the Buffer's area offset (`x`/`y`).
	///
	/// Usage discouraged, as it exposes `self.content` as a linearly indexable array, which limits
	/// potential future abstractions.
	///
	/// # Examples
	///
	/// ```
	/// use crate::buffer::Buffer;
	/// use crate::layout::Rect;
	///
	/// let rect = Rect::new(200, 100, 10, 10);
	/// let buffer = Buffer::empty(rect);
	/// assert_eq!(buffer.pos_of(0), (200, 100));
	/// assert_eq!(buffer.pos_of(14), (204, 101));
	/// ```
	///
	/// # Panics
	///
	/// Panics when given an index that is outside the Buffer's content.
	///
	/// ```should_panic
	/// use crate::buffer::Buffer;
	/// use crate::layout::Rect;
	///
	/// let rect = Rect::new(0, 0, 10, 10); // 100 cells in total
	/// let buffer = Buffer::empty(rect);
	/// // Index 100 is the 101th cell, which lies outside of the area of this Buffer.
	/// buffer.pos_of(100); // Panics
	/// ```
	#[must_use]
	pub fn pos_of(&self, index: usize) -> (u16, u16) {
		debug_assert!(
			index < self.content.len(),
			"Trying to get the coords of a cell outside the buffer: i={index} len={}",
			self.content.len()
		);
		let x = index % self.area.width as usize + self.area.x as usize;
		let y = index / self.area.width as usize + self.area.y as usize;
		(
			u16::try_from(x).expect("x overflow. This should never happen as area.width is u16"),
			u16::try_from(y).expect("y overflow. This should never happen as area.height is u16"),
		)
	}

	/// Print a string, starting at the position (x, y)
	pub fn set_string<T, S>(&mut self, x: u16, y: u16, string: T, style: S)
	where
		T: AsRef<str>,
		S: Into<Style>,
	{
		self.set_stringn(x, y, string, usize::MAX, style);
	}

	/// Print at most the first n characters of a string if enough space is available
	/// until the end of the line. Skips zero-width graphemes and control characters.
	///
	/// Use [`Buffer::set_string`] when the maximum amount of characters can be printed.
	pub fn set_stringn<T, S>(
		&mut self,
		mut x: u16,
		y: u16,
		string: T,
		max_width: usize,
		style: S,
	) -> (u16, u16)
	where
		T: AsRef<str>,
		S: Into<Style>,
	{
		let max_width = max_width.try_into().unwrap_or(u16::MAX);
		let mut remaining_width = self.area.right().saturating_sub(x).min(max_width);
		let graphemes = UnicodeSegmentation::graphemes(string.as_ref(), true)
			.filter(|symbol| !symbol.contains(char::is_control))
			.map(|symbol| (symbol, symbol.width() as u16))
			.filter(|(_symbol, width)| *width > 0)
			.map_while(|(symbol, width)| {
				remaining_width = remaining_width.checked_sub(width)?;
				Some((symbol, width))
			});
		let style = style.into();
		for (symbol, width) in graphemes {
			self[(x, y)].set_symbol(symbol).set_style(style);
			let next_symbol = x + width;
			x += 1;
			// Reset following cells if multi-width (they would be hidden by the grapheme),
			while x < next_symbol {
				self[(x, y)].reset();
				x += 1;
			}
		}
		(x, y)
	}

	/// Print a line, starting at the position (x, y)
	pub fn set_line(&mut self, x: u16, y: u16, line: &Line<'_>, max_width: u16) -> (u16, u16) {
		let mut remaining_width = max_width;
		let mut x = x;
		for span in line {
			if remaining_width == 0 {
				break;
			}
			let pos = self.set_stringn(
				x,
				y,
				span.content.as_ref(),
				remaining_width as usize,
				line.style.patch(span.style),
			);
			let w = pos.0.saturating_sub(x);
			x = pos.0;
			remaining_width = remaining_width.saturating_sub(w);
		}
		(x, y)
	}

	/// Print a span, starting at the position (x, y)
	pub fn set_span(&mut self, x: u16, y: u16, span: &Span<'_>, max_width: u16) -> (u16, u16) {
		self.set_stringn(x, y, &span.content, max_width as usize, span.style)
	}

	/// Set the style of all cells in the given area.
	///
	/// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
	/// your own type that implements [`Into<Style>`]).
	///
	/// [`Color`]: crate::style::Color
	pub fn set_style<S: Into<Style>>(&mut self, area: Rect, style: S) {
		let style = style.into();
		let area = self.area.intersection(area);
		for y in area.top()..area.bottom() {
			for x in area.left()..area.right() {
				self[(x, y)].set_style(style);
			}
		}
	}

	/// Resize the buffer so that the mapped area matches the given area and that the buffer
	/// length is equal to area.width * area.height
	pub fn resize(&mut self, area: Rect) {
		let length = area.area() as usize;
		if self.content.len() > length {
			self.content.truncate(length);
		} else {
			self.content.resize(length, Cell::EMPTY);
		}
		self.area = area;
	}

	/// Reset all cells in the buffer
	pub fn reset(&mut self) {
		for cell in &mut self.content {
			cell.reset();
		}
	}

	/// Merge an other buffer into this one
	pub fn merge(&mut self, other: &Self) {
		let area = self.area.union(other.area);
		self.content.resize(area.area() as usize, Cell::EMPTY);

		// Move original content to the appropriate space
		let size = self.area.area() as usize;
		for i in (0..size).rev() {
			let (x, y) = self.pos_of(i);
			// New index in content
			let k = ((y - area.y) * area.width + x - area.x) as usize;
			if i != k {
				self.content[k] = self.content[i].clone();
				self.content[i].reset();
			}
		}

		// Push content of the other buffer into this one (may erase previous
		// data)
		let size = other.area.area() as usize;
		for i in 0..size {
			let (x, y) = other.pos_of(i);
			// New index in content
			let k = ((y - area.y) * area.width + x - area.x) as usize;
			self.content[k] = other.content[i].clone();
		}
		self.area = area;
	}

	/// Builds a minimal sequence of coordinates and Cells necessary to update the UI from
	/// self to other.
	///
	/// We're assuming that buffers are well-formed, that is no double-width cell is followed by
	/// a non-blank cell.
	///
	/// # Multi-width characters handling:
	///
	/// ```text
	/// (Index:) `01`
	/// Prev:    `コ`
	/// Next:    `aa`
	/// Updates: `0: a, 1: a'
	/// ```
	///
	/// ```text
	/// (Index:) `01`
	/// Prev:    `a `
	/// Next:    `コ`
	/// Updates: `0: コ` (double width symbol at index 0 - skip index 1)
	/// ```
	///
	/// ```text
	/// (Index:) `012`
	/// Prev:    `aaa`
	/// Next:    `aコ`
	/// Updates: `0: a, 1: コ` (double width symbol at index 1 - skip index 2)
	/// ```
	pub fn diff<'a>(&self, other: &'a Self) -> Vec<(u16, u16, &'a Cell)> {
		let previous_buffer = &self.content;
		let next_buffer = &other.content;

		let mut updates: Vec<(u16, u16, &Cell)> = vec![];
		// Cells invalidated by drawing/replacing preceding multi-width characters:
		let mut invalidated: usize = 0;
		// Cells from the current buffer to skip due to preceding multi-width characters taking
		// their place (the skipped cells should be blank anyway), or due to per-cell-skipping:
		let mut to_skip: usize = 0;
		for (i, (current, previous)) in next_buffer.iter().zip(previous_buffer.iter()).enumerate() {
			if !current.skip && (current != previous || invalidated > 0) && to_skip == 0 {
				let (x, y) = self.pos_of(i);
				updates.push((x, y, &next_buffer[i]));

				// If the current cell is multi-width, ensure the trailing cells are explicitly
				// cleared when they previously contained non-blank content. Some terminals do not
				// reliably clear the trailing cell(s) when printing a wide grapheme, which can
				// result in visual artifacts (e.g., leftover characters). Emitting an explicit
				// update for the trailing cells avoids this.
				let symbol = current.symbol();
				let cell_width = symbol.width();
				// Work around terminals that fail to clear the trailing cell of certain
				// emoji presentation sequences (those containing VS16 / U+FE0F).
				// Only emit explicit clears for such sequences to avoid bloating diffs
				// for standard wide characters (e.g., CJK), which terminals handle well.
				let contains_vs16 = symbol.chars().any(|c| c == '\u{FE0F}');
				if cell_width > 1 && contains_vs16 {
					for k in 1..cell_width {
						let j = i + k;
						// Make sure that we are still inside the buffer.
						if j >= next_buffer.len() || j >= previous_buffer.len() {
							break;
						}
						let prev_trailing = &previous_buffer[j];
						let next_trailing = &next_buffer[j];
						if !next_trailing.skip && prev_trailing != next_trailing {
							let (tx, ty) = self.pos_of(j);
							// Push an explicit update for the trailing cell.
							// This is expected to be a blank cell, but we use the actual
							// content from the next buffer to handle cases where
							// the user has explicitly set something else.
							updates.push((tx, ty, next_trailing));
						}
					}
				}
			}

			to_skip = current.symbol().width().saturating_sub(1);

			let affected_width = cmp::max(current.symbol().width(), previous.symbol().width());
			invalidated = cmp::max(affected_width, invalidated).saturating_sub(1);
		}
		updates
	}
}

impl<P: Into<Position>> Index<P> for Buffer {
	type Output = Cell;

	/// Returns a reference to the [`Cell`] at the given position.
	///
	/// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
	/// `Position::new(x, y)`).
	///
	/// # Panics
	///
	/// May panic if the given position is outside the buffer's area. For a method that returns
	/// `None` instead of panicking, use [`Buffer::cell`](Self::cell).
	///
	/// # Examples
	///
	/// ```
	/// use crate::buffer::{Buffer, Cell};
	/// use crate::layout::{Position, Rect};
	///
	/// let buf = Buffer::empty(Rect::new(0, 0, 10, 10));
	/// let cell = &buf[(0, 0)];
	/// let cell = &buf[Position::new(0, 0)];
	/// ```
	fn index(&self, position: P) -> &Self::Output {
		let position = position.into();
		let index = self.index_of(position.x, position.y);
		&self.content[index]
	}
}

impl<P: Into<Position>> IndexMut<P> for Buffer {
	/// Returns a mutable reference to the [`Cell`] at the given position.
	///
	/// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
	/// `Position::new(x, y)`).
	///
	/// # Panics
	///
	/// May panic if the given position is outside the buffer's area. For a method that returns
	/// `None` instead of panicking, use [`Buffer::cell_mut`](Self::cell_mut).
	///
	/// # Examples
	///
	/// ```
	/// use crate::buffer::{Buffer, Cell};
	/// use crate::layout::{Position, Rect};
	///
	/// let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
	/// buf[(0, 0)].set_symbol("A");
	/// buf[Position::new(0, 0)].set_symbol("B");
	/// ```
	fn index_mut(&mut self, position: P) -> &mut Self::Output {
		let position = position.into();
		let index = self.index_of(position.x, position.y);
		&mut self.content[index]
	}
}

impl fmt::Debug for Buffer {
	/// Writes a debug representation of the buffer to the given formatter.
	///
	/// The format is like a pretty printed struct, with the following fields:
	/// * `area`: displayed as `Rect { x: 1, y: 2, width: 3, height: 4 }`
	/// * `content`: displayed as a list of strings representing the content of the buffer
	/// * `styles`: displayed as a list of: `{ x: 1, y: 2, fg: Color::Red, bg: Color::Blue,
	///   modifier: Modifier::BOLD }` only showing a value when there is a change in style.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_fmt(format_args!("Buffer {{\n    area: {:?}", &self.area))?;

		if self.area.is_empty() {
			return f.write_str("\n}");
		}

		f.write_str(",\n    content: [\n")?;
		let mut last_style = None;
		let mut styles = vec![];
		for (y, line) in self.content.chunks(self.area.width as usize).enumerate() {
			let mut overwritten = vec![];
			let mut skip: usize = 0;
			f.write_str("        \"")?;
			for (x, c) in line.iter().enumerate() {
				if skip == 0 {
					f.write_str(c.symbol())?;
				} else {
					overwritten.push((x, c.symbol()));
				}
				skip = cmp::max(skip, c.symbol().width()).saturating_sub(1);
				#[cfg(feature = "underline-color")]
				{
					let style = (c.fg, c.bg, c.underline_color, c.modifier);
					if last_style != Some(style) {
						last_style = Some(style);
						styles.push((x, y, c.fg, c.bg, c.underline_color, c.modifier));
					}
				}
				#[cfg(not(feature = "underline-color"))]
				{
					let style = (c.fg, c.bg, c.modifier);
					if last_style != Some(style) {
						last_style = Some(style);
						styles.push((x, y, c.fg, c.bg, c.modifier));
					}
				}
			}
			f.write_str("\",")?;
			if !overwritten.is_empty() {
				f.write_fmt(format_args!(
					" // hidden by multi-width symbols: {overwritten:?}"
				))?;
			}
			f.write_str("\n")?;
		}
		f.write_str("    ],\n    styles: [\n")?;
		for s in styles {
			#[cfg(feature = "underline-color")]
			f.write_fmt(format_args!(
				"        x: {}, y: {}, fg: {:?}, bg: {:?}, underline: {:?}, modifier: {:?},\n",
				s.0, s.1, s.2, s.3, s.4, s.5
			))?;
			#[cfg(not(feature = "underline-color"))]
			f.write_fmt(format_args!(
				"        x: {}, y: {}, fg: {:?}, bg: {:?}, modifier: {:?},\n",
				s.0, s.1, s.2, s.3, s.4
			))?;
		}
		f.write_str("    ]\n}")?;
		Ok(())
	}
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
