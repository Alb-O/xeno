//! Elements related to the `Block` base widget.
//!
//! This holds everything needed to display and configure a [`Block`].
//!
//! In its simplest form, a `Block` is a [border](Borders) around another widget. It can have a
//! [title](Block::title) and [padding](Block::padding).

use alloc::vec::Vec;

use itertools::Itertools;
use strum::{Display, EnumString};

pub use self::padding::Padding;
use crate::buffer::Buffer;
use crate::layout::{Alignment, Rect};
use crate::style::{Style, Styled};
use crate::symbols::border;
use crate::symbols::merge::MergeStrategy;
use crate::text::Line;
use crate::widgets::Widget;
use crate::widgets::borders::{BorderType, Borders};

mod padding;

/// Visual container with borders, titles, and padding for wrapping other widgets.
///
/// Most widgets accept an optional `Block` to provide visual framing. Use [`Block::inner`] to get
/// the content area after borders/padding. Styles layer: block style → border style → title style.
///
/// # Example
///
/// ```
/// use crate::text::Line;
/// use crate::widgets::{Block, Paragraph};
///
/// // Simple bordered block
/// let block = Block::bordered().title("My Block");
///
/// // Wrapping a widget
/// let paragraph = Paragraph::new("Hello").block(Block::bordered().title("Greeting"));
///
/// // Multiple titles with alignment
/// let block = Block::bordered()
///     .title_top(Line::from("Left").left_aligned())
///     .title_top(Line::from("Center").centered())
///     .title_bottom("Status: OK");
/// ```
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Block<'a> {
	/// List of titles
	titles: Vec<(Option<TitlePosition>, Line<'a>)>,
	/// The style to be patched to all titles of the block
	titles_style: Style,
	/// The default alignment of the titles that don't have one
	titles_alignment: Alignment,
	/// The default position of the titles that don't have one
	titles_position: TitlePosition,
	/// Visible borders
	borders: Borders,
	/// Border style
	border_style: Style,
	/// The symbols used to render the border. The default is plain lines but one can choose to
	/// have rounded or doubled lines instead or a custom set of symbols
	border_set: border::Set<'a>,
	/// Widget style
	style: Style,
	/// Block padding
	padding: Padding,
	/// Border merging strategy
	merge_borders: MergeStrategy,
}

/// Title position (top or bottom of block).
#[derive(Debug, Default, Display, EnumString, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitlePosition {
	/// Position the title at the top of the block.
	#[default]
	Top,
	/// Position the title at the bottom of the block.
	Bottom,
}

impl<'a> Block<'a> {
	/// Creates a new block with no [`Borders`] or [`Padding`].
	pub const fn new() -> Self {
		Self {
			titles: Vec::new(),
			titles_style: Style::new(),
			titles_alignment: Alignment::Left,
			titles_position: TitlePosition::Top,
			borders: Borders::NONE,
			border_style: Style::new(),
			border_set: BorderType::Padded.to_border_set(),
			style: Style::new(),
			padding: Padding::ZERO,
			merge_borders: MergeStrategy::Replace,
		}
	}

	/// Create a new block with all borders shown.
	pub const fn bordered() -> Self {
		let mut block = Self::new();
		block.borders = Borders::ALL;
		block
	}

	/// Adds a title using default position (top). Use [`Self::title_top`]/[`Self::title_bottom`] for explicit positioning.
	/// Multiple titles are space-separated. Accepts anything `Into<Line>`.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn title<T>(mut self, title: T) -> Self
	where
		T: Into<Line<'a>>,
	{
		self.titles.push((None, title.into()));
		self
	}

	/// Adds a title to the top of the block.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn title_top<T: Into<Line<'a>>>(mut self, title: T) -> Self {
		let line = title.into();
		self.titles.push((Some(TitlePosition::Top), line));
		self
	}

	/// Adds a title to the bottom of the block.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn title_bottom<T: Into<Line<'a>>>(mut self, title: T) -> Self {
		let line = title.into();
		self.titles.push((Some(TitlePosition::Bottom), line));
		self
	}

	/// Style applied to all titles (merged after block/border styles).
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn title_style<S: Into<Style>>(mut self, style: S) -> Self {
		self.titles_style = style.into();
		self
	}

	/// Default alignment for titles without explicit alignment.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn title_alignment(mut self, alignment: Alignment) -> Self {
		self.titles_alignment = alignment;
		self
	}

	/// Default position for titles added via [`Self::title`].
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn title_position(mut self, position: TitlePosition) -> Self {
		self.titles_position = position;
		self
	}

	/// Style for border areas (applied after block style).
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn border_style<S: Into<Style>>(mut self, style: S) -> Self {
		self.border_style = style.into();
		self
	}

	/// Base style for entire block. More specific styles (border, title) merge on top.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = style.into();
		self
	}

	/// Which borders to display. Use [`Self::bordered`] for all borders.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn borders(mut self, flag: Borders) -> Self {
		self.borders = flag;
		self
	}

	/// Border symbol style. See [`BorderType`] for options.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn border_type(mut self, border_type: BorderType) -> Self {
		self.border_set = border_type.to_border_set();
		self
	}

	/// Custom border symbols. Overwrites [`Self::border_type`].
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn border_set(mut self, border_set: border::Set<'a>) -> Self {
		self.border_set = border_set;
		self
	}

	/// Internal padding inside the block. See [`Padding`].
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn padding(mut self, padding: Padding) -> Self {
		self.padding = padding;
		self
	}

	/// Controls how borders merge with adjacent blocks. See [`MergeStrategy`].
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn merge_borders(mut self, strategy: MergeStrategy) -> Self {
		self.merge_borders = strategy;
		self
	}

	/// Computes the inner area after subtracting borders, titles, and padding.
	pub fn inner(&self, area: Rect) -> Rect {
		let mut inner = area;
		if self.borders.intersects(Borders::LEFT) {
			inner.x = inner.x.saturating_add(1).min(inner.right());
			inner.width = inner.width.saturating_sub(1);
		}
		if self.borders.intersects(Borders::TOP) || self.has_title_at_position(TitlePosition::Top) {
			inner.y = inner.y.saturating_add(1).min(inner.bottom());
			inner.height = inner.height.saturating_sub(1);
		}
		if self.borders.intersects(Borders::RIGHT) {
			inner.width = inner.width.saturating_sub(1);
		}
		if self.borders.intersects(Borders::BOTTOM)
			|| self.has_title_at_position(TitlePosition::Bottom)
		{
			inner.height = inner.height.saturating_sub(1);
		}

		inner.x = inner.x.saturating_add(self.padding.left);
		inner.y = inner.y.saturating_add(self.padding.top);

		inner.width = inner
			.width
			.saturating_sub(self.padding.left + self.padding.right);
		inner.height = inner
			.height
			.saturating_sub(self.padding.top + self.padding.bottom);

		inner
	}

	fn has_title_at_position(&self, position: TitlePosition) -> bool {
		self.titles
			.iter()
			.any(|(pos, _)| pos.unwrap_or(self.titles_position) == position)
	}
}

impl Widget for Block<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		Widget::render(&self, area, buf);
	}
}

impl Widget for &Block<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let area = area.intersection(buf.area);
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		self.render_borders(area, buf);
		self.render_titles(area, buf);
	}
}

impl Block<'_> {
	fn render_borders(&self, area: Rect, buf: &mut Buffer) {
		self.render_sides(area, buf);
		self.render_corners(area, buf);
	}

	fn render_sides(&self, area: Rect, buf: &mut Buffer) {
		let left = area.left();
		let top = area.top();
		// area.right() and area.bottom() are outside the rect, subtract 1 to get the last row/col
		let right = area.right() - 1;
		let bottom = area.bottom() - 1;

		// The first and last element of each line are not drawn when there is an adjacent line as
		// this would cause the corner to initially be merged with a side character and then a
		// corner character to be drawn on top of it. Some merge strategies would not produce a
		// correct character in that case.
		let is_replace = self.merge_borders != MergeStrategy::Replace;
		let left_inset = left + u16::from(is_replace && self.borders.contains(Borders::LEFT));
		let top_inset = top + u16::from(is_replace && self.borders.contains(Borders::TOP));
		let right_inset = right - u16::from(is_replace && self.borders.contains(Borders::RIGHT));
		let bottom_inset = bottom - u16::from(is_replace && self.borders.contains(Borders::BOTTOM));

		let sides = [
			(
				Borders::LEFT,
				left..=left,
				top_inset..=bottom_inset,
				self.border_set.vertical_left,
			),
			(
				Borders::TOP,
				left_inset..=right_inset,
				top..=top,
				self.border_set.horizontal_top,
			),
			(
				Borders::RIGHT,
				right..=right,
				top_inset..=bottom_inset,
				self.border_set.vertical_right,
			),
			(
				Borders::BOTTOM,
				left_inset..=right_inset,
				bottom..=bottom,
				self.border_set.horizontal_bottom,
			),
		];
		for (border, x_range, y_range, symbol) in sides {
			if self.borders.contains(border) {
				for x in x_range {
					for y in y_range.clone() {
						buf[(x, y)]
							.merge_symbol(symbol, self.merge_borders)
							.set_style(self.border_style);
					}
				}
			}
		}
	}

	fn render_corners(&self, area: Rect, buf: &mut Buffer) {
		let corners = [
			(
				Borders::RIGHT | Borders::BOTTOM,
				area.right() - 1,
				area.bottom() - 1,
				self.border_set.bottom_right,
			),
			(
				Borders::RIGHT | Borders::TOP,
				area.right() - 1,
				area.top(),
				self.border_set.top_right,
			),
			(
				Borders::LEFT | Borders::BOTTOM,
				area.left(),
				area.bottom() - 1,
				self.border_set.bottom_left,
			),
			(
				Borders::LEFT | Borders::TOP,
				area.left(),
				area.top(),
				self.border_set.top_left,
			),
		];

		for (border, x, y, symbol) in corners {
			if self.borders.contains(border) {
				buf[(x, y)]
					.merge_symbol(symbol, self.merge_borders)
					.set_style(self.border_style);
			}
		}
	}
	fn render_titles(&self, area: Rect, buf: &mut Buffer) {
		self.render_title_position(TitlePosition::Top, area, buf);
		self.render_title_position(TitlePosition::Bottom, area, buf);
	}

	fn render_title_position(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
		// NOTE: the order in which these functions are called defines the overlapping behavior
		self.render_left_titles(position, area, buf);
		self.render_center_titles(position, area, buf);
		self.render_right_titles(position, area, buf);
	}

	/// Render titles aligned to the right of the block
	///
	/// Currently (due to the way lines are truncated), the right side of the leftmost title will
	/// be cut off if the block is too small to fit all titles. This is not ideal and should be
	/// the left side of that leftmost that is cut off. This is due to the line being truncated
	/// incorrectly. See
	#[expect(clippy::similar_names)]
	fn render_right_titles(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
		let titles = self.filtered_titles(position, Alignment::Right);
		let mut titles_area = self.titles_area(area, position);

		// render titles in reverse order to align them to the right
		for title in titles.rev() {
			if titles_area.is_empty() {
				break;
			}
			let title_width = title.width() as u16;
			let title_area = Rect {
				x: titles_area
					.right()
					.saturating_sub(title_width)
					.max(titles_area.left()),
				width: title_width.min(titles_area.width),
				..titles_area
			};
			buf.set_style(title_area, self.titles_style);
			title.render(title_area, buf);

			// bump the width of the titles area to the left
			titles_area.width = titles_area
				.width
				.saturating_sub(title_width)
				.saturating_sub(1); // space between titles
		}
	}

	/// Render titles in the center of the block
	fn render_center_titles(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
		let area = self.titles_area(area, position);
		let titles = self
			.filtered_titles(position, Alignment::Center)
			.collect_vec();
		// titles are rendered with a space after each title except the last one
		let total_width = titles
			.iter()
			.map(|title| title.width() as u16 + 1)
			.sum::<u16>()
			.saturating_sub(1);

		if total_width <= area.width {
			self.render_centered_titles_without_truncation(titles, total_width, area, buf);
		} else {
			self.render_centered_titles_with_truncation(titles, total_width, area, buf);
		}
	}

	fn render_centered_titles_without_truncation(
		&self,
		titles: Vec<&Line<'_>>,
		total_width: u16,
		area: Rect,
		buf: &mut Buffer,
	) {
		// titles fit in the area, center them
		let x = area.left() + area.width.saturating_sub(total_width) / 2;
		let mut area = Rect { x, ..area };
		for title in titles {
			let width = title.width() as u16;
			let title_area = Rect { width, ..area };
			buf.set_style(title_area, self.titles_style);
			title.render(title_area, buf);
			// Move the rendering cursor to the right, leaving 1 column space.
			area.x = area.x.saturating_add(width + 1);
			area.width = area.width.saturating_sub(width + 1);
		}
	}

	fn render_centered_titles_with_truncation(
		&self,
		titles: Vec<&Line<'_>>,
		total_width: u16,
		mut area: Rect,
		buf: &mut Buffer,
	) {
		// titles do not fit in the area, truncate the left side using an offset. The right side
		// is truncated by the area width.
		let mut offset = total_width.saturating_sub(area.width) / 2;
		for title in titles {
			if area.is_empty() {
				break;
			}
			let width = area.width.min(title.width() as u16).saturating_sub(offset);
			let title_area = Rect { width, ..area };
			buf.set_style(title_area, self.titles_style);
			if offset > 0 {
				// truncate the left side of the title to fit the area
				title.clone().right_aligned().render(title_area, buf);
				offset = offset.saturating_sub(width).saturating_sub(1);
			} else {
				// truncate the right side of the title to fit the area if needed
				title.clone().left_aligned().render(title_area, buf);
			}
			// Leave 1 column of spacing between titles.
			area.x = area.x.saturating_add(width + 1);
			area.width = area.width.saturating_sub(width + 1);
		}
	}

	/// Render titles aligned to the left of the block
	#[expect(clippy::similar_names)]
	fn render_left_titles(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
		let titles = self.filtered_titles(position, Alignment::Left);
		let mut titles_area = self.titles_area(area, position);
		for title in titles {
			if titles_area.is_empty() {
				break;
			}
			let title_width = title.width() as u16;
			let title_area = Rect {
				width: title_width.min(titles_area.width),
				..titles_area
			};
			buf.set_style(title_area, self.titles_style);
			title.render(title_area, buf);

			// bump the titles area to the right and reduce its width
			titles_area.x = titles_area.x.saturating_add(title_width + 1);
			titles_area.width = titles_area.width.saturating_sub(title_width + 1);
		}
	}

	/// An iterator over the titles that match the position and alignment
	fn filtered_titles(
		&self,
		position: TitlePosition,
		alignment: Alignment,
	) -> impl DoubleEndedIterator<Item = &Line<'_>> {
		self.titles
			.iter()
			.filter(move |(pos, _)| pos.unwrap_or(self.titles_position) == position)
			.filter(move |(_, line)| line.alignment.unwrap_or(self.titles_alignment) == alignment)
			.map(|(_, line)| line)
	}

	/// An area that is one line tall and spans the width of the block excluding the borders and
	/// is positioned at the top or bottom of the block.
	fn titles_area(&self, area: Rect, position: TitlePosition) -> Rect {
		let left_border = u16::from(self.borders.contains(Borders::LEFT));
		let right_border = u16::from(self.borders.contains(Borders::RIGHT));
		Rect {
			x: area.left() + left_border,
			y: match position {
				TitlePosition::Top => area.top(),
				TitlePosition::Bottom => area.bottom() - 1,
			},
			width: area
				.width
				.saturating_sub(left_border)
				.saturating_sub(right_border),
			height: 1,
		}
	}

	/// Calculate the left, and right space the [`Block`] will take up.
	///
	/// The result takes the [`Block`]'s, [`Borders`], and [`Padding`] into account.
	pub(crate) fn horizontal_space(&self) -> (u16, u16) {
		let left = self
			.padding
			.left
			.saturating_add(u16::from(self.borders.contains(Borders::LEFT)));
		let right = self
			.padding
			.right
			.saturating_add(u16::from(self.borders.contains(Borders::RIGHT)));
		(left, right)
	}

	/// Calculate the top, and bottom space that the [`Block`] will take up.
	///
	/// Takes the [`Padding`], [`TitlePosition`], and the [`Borders`] that are selected into
	/// account when calculating the result.
	pub(crate) fn vertical_space(&self) -> (u16, u16) {
		let has_top =
			self.borders.contains(Borders::TOP) || self.has_title_at_position(TitlePosition::Top);
		let top = self.padding.top + u16::from(has_top);
		let has_bottom = self.borders.contains(Borders::BOTTOM)
			|| self.has_title_at_position(TitlePosition::Bottom);
		let bottom = self.padding.bottom + u16::from(has_bottom);
		(top, bottom)
	}
}

/// An extension trait for [`Block`] that provides some convenience methods.
///
/// This is implemented for [`Option<Block>`](Option) to simplify the common case of having a
/// widget with an optional block.
pub trait BlockExt {
	/// Return the inner area of the block if it is `Some`. Otherwise, returns `area`.
	///
	/// This is a useful convenience method for widgets that have an `Option<Block>` field
	fn inner_if_some(&self, area: Rect) -> Rect;
}

impl BlockExt for Option<Block<'_>> {
	fn inner_if_some(&self, area: Rect) -> Rect {
		self.as_ref().map_or(area, |block| block.inner(area))
	}
}

impl Styled for Block<'_> {
	type Item = Self;

	fn style(&self) -> Style {
		self.style
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		self.style(style)
	}
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
