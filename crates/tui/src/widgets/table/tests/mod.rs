use alloc::string::ToString;
use alloc::{format, vec};

use rstest::{fixture, rstest};

use super::*;
use crate::layout::Constraint::*;
use crate::style::{Color, Modifier, Style, Stylize};
use crate::text::Line;
use crate::widgets::borders::BorderType;
use crate::widgets::table::Cell;

#[test]
fn new() {
	let rows = [Row::new(vec![Cell::from("")])];
	let widths = [Constraint::Percentage(100)];
	let table = Table::new(rows.clone(), widths);
	assert_eq!(table.rows, rows);
	assert_eq!(table.header, None);
	assert_eq!(table.footer, None);
	assert_eq!(table.widths, widths);
	assert_eq!(table.column_spacing, 1);
	assert_eq!(table.block, None);
	assert_eq!(table.style, Style::default());
	assert_eq!(table.row_highlight_style, Style::default());
	assert_eq!(table.highlight_symbol, Text::default());
	assert_eq!(table.highlight_spacing, HighlightSpacing::WhenSelected);
	assert_eq!(table.flex, Flex::Start);
}

#[test]
fn default() {
	let table = Table::default();
	assert_eq!(table.rows, []);
	assert_eq!(table.header, None);
	assert_eq!(table.footer, None);
	assert_eq!(table.widths, []);
	assert_eq!(table.column_spacing, 1);
	assert_eq!(table.block, None);
	assert_eq!(table.style, Style::default());
	assert_eq!(table.row_highlight_style, Style::default());
	assert_eq!(table.highlight_symbol, Text::default());
	assert_eq!(table.highlight_spacing, HighlightSpacing::WhenSelected);
	assert_eq!(table.flex, Flex::Start);
}

#[test]
fn collect() {
	let table = (0..4)
		.map(|i| -> Row { (0..4).map(|j| format!("{i}*{j} = {}", i * j)).collect() })
		.collect::<Table>()
		.widths([Constraint::Percentage(25); 4]);

	let expected_rows: Vec<Row> = vec![
		Row::new(["0*0 = 0", "0*1 = 0", "0*2 = 0", "0*3 = 0"]),
		Row::new(["1*0 = 0", "1*1 = 1", "1*2 = 2", "1*3 = 3"]),
		Row::new(["2*0 = 0", "2*1 = 2", "2*2 = 4", "2*3 = 6"]),
		Row::new(["3*0 = 0", "3*1 = 3", "3*2 = 6", "3*3 = 9"]),
	];

	assert_eq!(table.rows, expected_rows);
	assert_eq!(table.widths, [Constraint::Percentage(25); 4]);
}

#[test]
fn widths() {
	let table = Table::default().widths([Constraint::Length(100)]);
	assert_eq!(table.widths, [Constraint::Length(100)]);

	// ensure that code that uses &[] continues to work as there is a large amount of code that
	// uses this pattern
	#[expect(clippy::needless_borrows_for_generic_args)]
	let table = Table::default().widths(&[Constraint::Length(100)]);
	assert_eq!(table.widths, [Constraint::Length(100)]);

	let table = Table::default().widths(vec![Constraint::Length(100)]);
	assert_eq!(table.widths, [Constraint::Length(100)]);

	// ensure that code that uses &some_vec continues to work as there is a large amount of code
	// that uses this pattern
	#[expect(clippy::needless_borrows_for_generic_args)]
	let table = Table::default().widths(&vec![Constraint::Length(100)]);
	assert_eq!(table.widths, [Constraint::Length(100)]);

	let table = Table::default().widths([100].into_iter().map(Constraint::Length));
	assert_eq!(table.widths, [Constraint::Length(100)]);
}

#[test]
fn rows() {
	let rows = [Row::new(vec![Cell::from("")])];
	let table = Table::default().rows(rows.clone());
	assert_eq!(table.rows, rows);
}

#[test]
fn column_spacing() {
	let table = Table::default().column_spacing(2);
	assert_eq!(table.column_spacing, 2);
}

#[test]
fn block() {
	let block = Block::bordered().title("Table");
	let table = Table::default().block(block.clone());
	assert_eq!(table.block, Some(block));
}

#[test]
fn header() {
	let header = Row::new(vec![Cell::from("")]);
	let table = Table::default().header(header.clone());
	assert_eq!(table.header, Some(header));
}

#[test]
fn footer() {
	let footer = Row::new(vec![Cell::from("")]);
	let table = Table::default().footer(footer.clone());
	assert_eq!(table.footer, Some(footer));
}

#[test]
#[expect(deprecated)]
fn highlight_style() {
	let style = Style::default().red().italic();
	let table = Table::default().highlight_style(style);
	assert_eq!(table.row_highlight_style, style);
}

#[test]
fn row_highlight_style() {
	let style = Style::default().red().italic();
	let table = Table::default().row_highlight_style(style);
	assert_eq!(table.row_highlight_style, style);
}

#[test]
fn column_highlight_style() {
	let style = Style::default().red().italic();
	let table = Table::default().column_highlight_style(style);
	assert_eq!(table.column_highlight_style, style);
}

#[test]
fn cell_highlight_style() {
	let style = Style::default().red().italic();
	let table = Table::default().cell_highlight_style(style);
	assert_eq!(table.cell_highlight_style, style);
}

#[test]
fn highlight_symbol() {
	let table = Table::default().highlight_symbol(">>");
	assert_eq!(table.highlight_symbol, Text::from(">>"));
}

#[test]
fn highlight_spacing() {
	let table = Table::default().highlight_spacing(HighlightSpacing::Always);
	assert_eq!(table.highlight_spacing, HighlightSpacing::Always);
}

#[test]
#[should_panic = "Percentages should be between 0 and 100 inclusively"]
fn table_invalid_percentages() {
	let _ = Table::default().widths([Constraint::Percentage(110)]);
}

#[test]
fn widths_conversions() {
	let array = [Constraint::Percentage(100)];
	let table = Table::new(Vec::<Row>::new(), array);
	assert_eq!(table.widths, [Constraint::Percentage(100)], "array");

	let array_ref = &[Constraint::Percentage(100)];
	let table = Table::new(Vec::<Row>::new(), array_ref);
	assert_eq!(table.widths, [Constraint::Percentage(100)], "array ref");

	let vec = vec![Constraint::Percentage(100)];
	let slice = vec.as_slice();
	let table = Table::new(Vec::<Row>::new(), slice);
	assert_eq!(table.widths, [Constraint::Percentage(100)], "slice");

	let vec = vec![Constraint::Percentage(100)];
	let table = Table::new(Vec::<Row>::new(), vec);
	assert_eq!(table.widths, [Constraint::Percentage(100)], "vec");

	let vec_ref = &vec![Constraint::Percentage(100)];
	let table = Table::new(Vec::<Row>::new(), vec_ref);
	assert_eq!(table.widths, [Constraint::Percentage(100)], "vec ref");
}

mod column_widths;
mod render;
mod state;
