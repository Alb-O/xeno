//! The [`Table`] widget is used to display multiple rows and columns in a grid and allows selecting
//! one or multiple cells.

use alloc::vec;
use alloc::vec::Vec;

use itertools::Itertools;

pub use self::cell::Cell;
pub use self::highlight_spacing::HighlightSpacing;
pub use self::row::Row;
pub use self::state::TableState;
use crate::buffer::Buffer;
use crate::layout::{Constraint, Flex, Layout, Rect};
use crate::style::{Style, Styled};
use crate::text::Text;
use crate::widgets::block::{Block, BlockExt};
use crate::widgets::{StatefulWidget, Widget};

mod cell;
mod highlight_spacing;
mod row;
mod state;

/// A widget to display data in formatted columns.
///
/// A `Table` is a collection of [`Row`]s, each composed of [`Cell`]s. Construct with
/// [`Table::new`] and chain builder methods. Use [`Table::widths`] to set column widths;
/// without it columns default to equal widths.
///
/// Implements both [`Widget`] and [`StatefulWidget`]. With [`TableState`], supports row/column
/// selection with automatic scrolling. Highlight styles apply in order: Row, Column, Cell.
///
/// # Example
///
/// ```rust
/// use crate::layout::Constraint;
/// use crate::style::{Style, Stylize};
/// use crate::widgets::{Block, Row, Table, TableState};
///
/// let rows = [
///     Row::new(vec!["Cell1", "Cell2"]),
///     Row::new(vec!["Cell3", "Cell4"]),
/// ];
/// let table = Table::new(rows, [Constraint::Length(5); 2])
///     .header(Row::new(vec!["Col1", "Col2"]).style(Style::new().bold()).bottom_margin(1))
///     .footer(Row::new(vec!["Footer"]))
///     .block(Block::new().title("Table"))
///     .row_highlight_style(Style::new().reversed())
///     .highlight_symbol(">>");
///
/// // For stateful usage, store TableState in your app state
/// let mut state = TableState::default();
/// // frame.render_stateful_widget(table, area, &mut state);
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Table<'a> {
	/// Data to display in each row
	rows: Vec<Row<'a>>,

	/// Optional header
	header: Option<Row<'a>>,

	/// Optional footer
	footer: Option<Row<'a>>,

	/// Width constraints for each column
	widths: Vec<Constraint>,

	/// Space between each column
	column_spacing: u16,

	/// A block to wrap the widget in
	block: Option<Block<'a>>,

	/// Base style for the widget
	style: Style,

	/// Style used to render the selected row
	row_highlight_style: Style,

	/// Style used to render the selected column
	column_highlight_style: Style,

	/// Style used to render the selected cell
	cell_highlight_style: Style,

	/// Symbol in front of the selected row
	highlight_symbol: Text<'a>,

	/// Decides when to allocate spacing for the row selection
	highlight_spacing: HighlightSpacing,

	/// Controls how to distribute extra space among the columns
	flex: Flex,
}

impl Default for Table<'_> {
	fn default() -> Self {
		Self {
			rows: Vec::new(),
			header: None,
			footer: None,
			widths: Vec::new(),
			column_spacing: 1,
			block: None,
			style: Style::new(),
			row_highlight_style: Style::new(),
			column_highlight_style: Style::new(),
			cell_highlight_style: Style::new(),
			highlight_symbol: Text::default(),
			highlight_spacing: HighlightSpacing::default(),
			flex: Flex::Start,
		}
	}
}

impl<'a> Table<'a> {
	/// Creates a new [`Table`] widget with the given rows.
	///
	/// The `rows` parameter accepts any value that can be converted into an iterator of [`Row`]s.
	/// This includes arrays, slices, and [`Vec`]s.
	///
	/// The `widths` parameter accepts any type that implements `IntoIterator<Item =
	/// Into<Constraint>>`. This includes arrays, slices, vectors, iterators. `Into<Constraint>` is
	/// implemented on u16, so you can pass an array, vec, etc. of u16 to this function to create a
	/// table with fixed width columns.
	///
	/// # Examples
	///
	/// ```rust
	/// use crate::layout::Constraint;
	/// use crate::widgets::{Row, Table};
	///
	/// let rows = [
	///     Row::new(vec!["Cell1", "Cell2"]),
	///     Row::new(vec!["Cell3", "Cell4"]),
	/// ];
	/// let widths = [Constraint::Length(5), Constraint::Length(5)];
	/// let table = Table::new(rows, widths);
	/// ```
	pub fn new<R, C>(rows: R, widths: C) -> Self
	where
		R: IntoIterator,
		R::Item: Into<Row<'a>>,
		C: IntoIterator,
		C::Item: Into<Constraint>,
	{
		let widths = widths.into_iter().map(Into::into).collect_vec();
		ensure_percentages_less_than_100(&widths);

		let rows = rows.into_iter().map(Into::into).collect();
		Self {
			rows,
			widths,
			..Default::default()
		}
	}

	/// Set the rows. Does not set column widths; call [`Table::widths`] separately.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn rows<T>(mut self, rows: T) -> Self
	where
		T: IntoIterator<Item = Row<'a>>,
	{
		self.rows = rows.into_iter().collect();
		self
	}

	/// Sets the header row, displayed at the top of the table.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn header(mut self, header: Row<'a>) -> Self {
		self.header = Some(header);
		self
	}

	/// Sets the footer row, displayed at the bottom of the table.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn footer(mut self, footer: Row<'a>) -> Self {
		self.footer = Some(footer);
		self
	}

	/// Set the widths of the columns. Accepts arrays, slices, vectors, or iterators of
	/// [`Constraint`]. Also accepts `u16` values via `Into<Constraint>`. Empty widths
	/// result in equal column distribution.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn widths<I>(mut self, widths: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		let widths = widths.into_iter().map(Into::into).collect_vec();
		ensure_percentages_less_than_100(&widths);
		self.widths = widths;
		self
	}

	/// Set the spacing between columns.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn column_spacing(mut self, spacing: u16) -> Self {
		self.column_spacing = spacing;
		self
	}

	/// Wraps the table with a [`Block`] widget.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Sets the base style. Overridden by [`Block::style`], [`Row::style`], [`Cell::style`].
	/// Also implements [`Styled`] trait for shorthand like `.red().italic()`.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
		self.style = style.into();
		self
	}

	/// Deprecated: use [`Self::row_highlight_style`] instead.
	#[must_use = "method moves the value of self and returns the modified value"]
	#[deprecated(note = "use `row_highlight_style()` instead")]
	pub fn highlight_style<S: Into<Style>>(self, highlight_style: S) -> Self {
		self.row_highlight_style(highlight_style)
	}

	/// Style for selected row. Overrides row/cell styles, includes selection symbol.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn row_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
		self.row_highlight_style = highlight_style.into();
		self
	}

	/// Style for selected column. Overrides row/cell styles.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn column_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
		self.column_highlight_style = highlight_style.into();
		self
	}

	/// Style for selected cell. Overrides row/cell styles.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn cell_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
		self.cell_highlight_style = highlight_style.into();
		self
	}

	/// Symbol displayed in front of the selected row (e.g. ">>").
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn highlight_symbol<T: Into<Text<'a>>>(mut self, highlight_symbol: T) -> Self {
		self.highlight_symbol = highlight_symbol.into();
		self
	}

	/// Controls when selection symbol column space is allocated.
	/// - `Always`: constant width, no layout shift on selection
	/// - `WhenSelected`: allocates only when row selected (default, may cause shift)
	/// - `Never`: no space allocated, symbol never drawn
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn highlight_spacing(mut self, value: HighlightSpacing) -> Self {
		self.highlight_spacing = value;
		self
	}

	/// Controls extra space distribution among columns (default: `Flex::Start`).
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn flex(mut self, flex: Flex) -> Self {
		self.flex = flex;
		self
	}
}

impl Widget for Table<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		Widget::render(&self, area, buf);
	}
}

impl Widget for &Table<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut state = TableState::default();
		StatefulWidget::render(self, area, buf, &mut state);
	}
}

impl StatefulWidget for Table<'_> {
	type State = TableState;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		StatefulWidget::render(&self, area, buf, state);
	}
}

impl StatefulWidget for &Table<'_> {
	type State = TableState;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		buf.set_style(area, self.style);
		self.block.as_ref().render(area, buf);
		let table_area = self.block.inner_if_some(area);
		if table_area.is_empty() {
			return;
		}

		if state.selected.is_some_and(|s| s >= self.rows.len()) {
			state.select(Some(self.rows.len().saturating_sub(1)));
		}

		if self.rows.is_empty() {
			state.select(None);
		}

		let column_count = self.column_count();
		if state.selected_column.is_some_and(|s| s >= column_count) {
			state.select_column(Some(column_count.saturating_sub(1)));
		}
		if column_count == 0 {
			state.select_column(None);
		}

		let selection_width = self.selection_width(state);
		let column_widths = self.get_column_widths(table_area.width, selection_width, column_count);
		let (header_area, rows_area, footer_area) = self.layout(table_area);

		self.render_header(header_area, buf, &column_widths);

		self.render_rows(rows_area, buf, state, selection_width, &column_widths);

		self.render_footer(footer_area, buf, &column_widths);
	}
}

// private methods for rendering
impl Table<'_> {
	/// Splits the table area into a header, rows area and a footer
	fn layout(&self, area: Rect) -> (Rect, Rect, Rect) {
		let header_top_margin = self.header.as_ref().map_or(0, |h| h.top_margin);
		let header_height = self.header.as_ref().map_or(0, |h| h.height);
		let header_bottom_margin = self.header.as_ref().map_or(0, |h| h.bottom_margin);
		let footer_top_margin = self.footer.as_ref().map_or(0, |h| h.top_margin);
		let footer_height = self.footer.as_ref().map_or(0, |f| f.height);
		let footer_bottom_margin = self.footer.as_ref().map_or(0, |h| h.bottom_margin);
		let layout = Layout::vertical([
			Constraint::Length(header_top_margin),
			Constraint::Length(header_height),
			Constraint::Length(header_bottom_margin),
			Constraint::Min(0),
			Constraint::Length(footer_top_margin),
			Constraint::Length(footer_height),
			Constraint::Length(footer_bottom_margin),
		])
		.split(area);
		let (header_area, rows_area, footer_area) = (layout[1], layout[3], layout[5]);
		(header_area, rows_area, footer_area)
	}

	fn render_header(&self, area: Rect, buf: &mut Buffer, column_widths: &[(u16, u16)]) {
		if let Some(ref header) = self.header {
			buf.set_style(area, header.style);
			for ((x, width), cell) in column_widths.iter().zip(header.cells.iter()) {
				cell.render(Rect::new(area.x + x, area.y, *width, area.height), buf);
			}
		}
	}

	fn render_footer(&self, area: Rect, buf: &mut Buffer, column_widths: &[(u16, u16)]) {
		if let Some(ref footer) = self.footer {
			buf.set_style(area, footer.style);
			for ((x, width), cell) in column_widths.iter().zip(footer.cells.iter()) {
				cell.render(Rect::new(area.x + x, area.y, *width, area.height), buf);
			}
		}
	}

	fn render_rows(
		&self,
		area: Rect,
		buf: &mut Buffer,
		state: &mut TableState,
		selection_width: u16,
		columns_widths: &[(u16, u16)],
	) {
		if self.rows.is_empty() {
			return;
		}

		let (start_index, end_index) = self.visible_rows(state, area);
		state.offset = start_index;

		let mut y_offset = 0;

		let mut selected_row_area = None;
		for (i, row) in self
			.rows
			.iter()
			.enumerate()
			.skip(start_index)
			.take(end_index - start_index)
		{
			let y = area.y + y_offset + row.top_margin;
			let height = (y + row.height).min(area.bottom()).saturating_sub(y);
			let row_area = Rect { y, height, ..area };
			buf.set_style(row_area, row.style);

			let is_selected = state.selected.is_some_and(|index| index == i);
			if selection_width > 0 && is_selected {
				let selection_area = Rect {
					width: selection_width,
					..row_area
				};
				buf.set_style(selection_area, row.style);
				(&self.highlight_symbol).render(selection_area, buf);
			}
			for ((x, width), cell) in columns_widths.iter().zip(row.cells.iter()) {
				cell.render(
					Rect::new(row_area.x + x, row_area.y, *width, row_area.height),
					buf,
				);
			}
			if is_selected {
				selected_row_area = Some(row_area);
			}
			y_offset += row.height_with_margin();
		}

		let selected_column_area = state.selected_column.and_then(|s| {
			// The selection is clamped by the column count. Since a user can manually specify an
			// incorrect number of widths, we should use panic free methods.
			columns_widths.get(s).map(|(x, width)| Rect {
				x: x + area.x,
				width: *width,
				..area
			})
		});

		match (selected_row_area, selected_column_area) {
			(Some(row_area), Some(col_area)) => {
				buf.set_style(row_area, self.row_highlight_style);
				buf.set_style(col_area, self.column_highlight_style);
				let cell_area = row_area.intersection(col_area);
				buf.set_style(cell_area, self.cell_highlight_style);
			}
			(Some(row_area), None) => {
				buf.set_style(row_area, self.row_highlight_style);
			}
			(None, Some(col_area)) => {
				buf.set_style(col_area, self.column_highlight_style);
			}
			(None, None) => (),
		}
	}

	/// Return the indexes of the visible rows.
	///
	/// The algorithm works as follows:
	/// - start at the offset and calculate the height of the rows that can be displayed within the
	///   area.
	/// - if the selected row is not visible, scroll the table to ensure it is visible.
	/// - if there is still space to fill then there's a partial row at the end which should be
	///   included in the view.
	fn visible_rows(&self, state: &TableState, area: Rect) -> (usize, usize) {
		let last_row = self.rows.len().saturating_sub(1);
		let mut start = state.offset.min(last_row);

		if let Some(selected) = state.selected {
			start = start.min(selected);
		}

		let mut end = start;
		let mut height = 0;

		for item in self.rows.iter().skip(start) {
			if height + item.height > area.height {
				break;
			}
			height += item.height_with_margin();
			end += 1;
		}

		if let Some(selected) = state.selected {
			let selected = selected.min(last_row);

			// scroll down until the selected row is visible
			while selected >= end {
				height = height.saturating_add(self.rows[end].height_with_margin());
				end += 1;
				while height > area.height {
					height = height.saturating_sub(self.rows[start].height_with_margin());
					start += 1;
				}
			}
		}

		// Include a partial row if there is space
		if height < area.height && end < self.rows.len() {
			end += 1;
		}

		(start, end)
	}

	/// Get all offsets and widths of all user specified columns.
	///
	/// Returns (x, width). When self.widths is empty, it is assumed `.widths()` has not been called
	/// and a default of equal widths is returned.
	fn get_column_widths(
		&self,
		max_width: u16,
		selection_width: u16,
		col_count: usize,
	) -> Vec<(u16, u16)> {
		let widths = if self.widths.is_empty() {
			// Divide the space between each column equally
			vec![Constraint::Length(max_width / col_count.max(1) as u16); col_count]
		} else {
			self.widths.clone()
		};
		// this will always allocate a selection area
		let [_selection_area, columns_area] =
			Layout::horizontal([Constraint::Length(selection_width), Constraint::Fill(0)])
				.areas(Rect::new(0, 0, max_width, 1));
		let rects = Layout::horizontal(widths)
			.flex(self.flex)
			.spacing(self.column_spacing)
			.split(columns_area);
		rects.iter().map(|c| (c.x, c.width)).collect()
	}

	fn column_count(&self) -> usize {
		self.rows
			.iter()
			.chain(self.footer.iter())
			.chain(self.header.iter())
			.map(|r| r.cells.len())
			.max()
			.unwrap_or_default()
	}

	/// Returns the width of the selection column if a row is selected, or the `highlight_spacing`
	/// is set to show the column always, otherwise 0.
	fn selection_width(&self, state: &TableState) -> u16 {
		let has_selection = state.selected.is_some();
		if self.highlight_spacing.should_add(has_selection) {
			self.highlight_symbol.width() as u16
		} else {
			0
		}
	}
}

fn ensure_percentages_less_than_100(widths: &[Constraint]) {
	for w in widths {
		if let Constraint::Percentage(p) = w {
			assert!(
				*p <= 100,
				"Percentages should be between 0 and 100 inclusively."
			);
		}
	}
}

impl Styled for Table<'_> {
	type Item = Self;

	fn style(&self) -> Style {
		self.style
	}

	fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
		self.style(style)
	}
}

impl<'a, Item> FromIterator<Item> for Table<'a>
where
	Item: Into<Row<'a>>,
{
	/// Collects an iterator of rows into a table.
	///
	/// When collecting from an iterator into a table, the user must provide the widths using
	/// `Table::widths` after construction.
	fn from_iter<Iter: IntoIterator<Item = Item>>(rows: Iter) -> Self {
		let widths: [Constraint; 0] = [];
		Self::new(rows, widths)
	}
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
