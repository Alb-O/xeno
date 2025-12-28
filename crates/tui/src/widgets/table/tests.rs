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

#[cfg(test)]
mod state {
	use super::*;
	use crate::buffer::Buffer;
	use crate::layout::{Constraint, Rect};
	use crate::widgets::StatefulWidget;
	use crate::widgets::table::{Row, Table, TableState};

	#[fixture]
	fn table_buf() -> Buffer {
		Buffer::empty(Rect::new(0, 0, 10, 10))
	}

	#[rstest]
	fn test_list_state_empty_list(mut table_buf: Buffer) {
		let mut state = TableState::default();

		let rows: Vec<Row> = Vec::new();
		let widths = vec![Constraint::Percentage(100)];
		let table = Table::new(rows, widths);
		state.select_first();
		StatefulWidget::render(table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected, None);
		assert_eq!(state.selected_column, None);
	}

	#[rstest]
	fn test_list_state_single_item(mut table_buf: Buffer) {
		let mut state = TableState::default();

		let widths = vec![Constraint::Percentage(100)];

		let items = vec![Row::new(vec!["Item 1"])];
		let table = Table::new(items, widths);
		state.select_first();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected, Some(0));
		assert_eq!(state.selected_column, None);

		state.select_last();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected, Some(0));
		assert_eq!(state.selected_column, None);

		state.select_previous();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected, Some(0));
		assert_eq!(state.selected_column, None);

		state.select_next();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected, Some(0));
		assert_eq!(state.selected_column, None);

		let mut state = TableState::default();

		state.select_first_column();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected_column, Some(0));
		assert_eq!(state.selected, None);

		state.select_last_column();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected_column, Some(0));
		assert_eq!(state.selected, None);

		state.select_previous_column();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected_column, Some(0));
		assert_eq!(state.selected, None);

		state.select_next_column();
		StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
		assert_eq!(state.selected_column, Some(0));
		assert_eq!(state.selected, None);
	}
}

#[cfg(test)]
mod render {
	use super::*;
	use crate::layout::Alignment;

	#[test]
	fn render_empty_area() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let rows = vec![Row::new(vec!["Cell1", "Cell2"])];
		let table = Table::new(rows, vec![Constraint::Length(5); 2]);
		Widget::render(table, Rect::new(0, 0, 0, 0), &mut buf);
		assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 15, 3)));
	}

	#[test]
	fn render_default() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let table = Table::default();
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 15, 3)));
	}

	#[test]
	fn render_with_block() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let block = Block::bordered()
			.border_type(BorderType::Plain)
			.title("Block");
		let table = Table::new(rows, vec![Constraint::Length(5); 2]).block(block);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "┌Block────────┐",
                "│Cell1 Cell2  │",
                "└─────────────┘",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_header() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let header = Row::new(vec!["Head1", "Head2"]);
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 2]).header(header);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Head1 Head2    ",
                "Cell1 Cell2    ",
                "Cell3 Cell4    ",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_footer() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let footer = Row::new(vec!["Foot1", "Foot2"]);
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 2]).footer(footer);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Cell1 Cell2    ",
                "Cell3 Cell4    ",
                "Foot1 Foot2    ",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_header_and_footer() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let header = Row::new(vec!["Head1", "Head2"]);
		let footer = Row::new(vec!["Foot1", "Foot2"]);
		let rows = vec![Row::new(vec!["Cell1", "Cell2"])];
		let table = Table::new(rows, [Constraint::Length(5); 2])
			.header(header)
			.footer(footer);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Head1 Head2    ",
                "Cell1 Cell2    ",
                "Foot1 Foot2    ",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_header_margin() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let header = Row::new(vec!["Head1", "Head2"]).bottom_margin(1);
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 2]).header(header);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Head1 Head2    ",
                "               ",
                "Cell1 Cell2    ",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_footer_margin() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let footer = Row::new(vec!["Foot1", "Foot2"]).top_margin(1);
		let rows = vec![Row::new(vec!["Cell1", "Cell2"])];
		let table = Table::new(rows, [Constraint::Length(5); 2]).footer(footer);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Cell1 Cell2    ",
                "               ",
                "Foot1 Foot2    ",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_row_margin() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]).bottom_margin(1),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 2]);
		Widget::render(table, Rect::new(0, 0, 15, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Cell1 Cell2    ",
                "               ",
                "Cell3 Cell4    ",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_tall_row() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 23, 3));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec![
				Text::raw("Cell3-Line1\nCell3-Line2\nCell3-Line3"),
				Text::raw("Cell4-Line1\nCell4-Line2\nCell4-Line3"),
			])
			.height(3),
		];
		let table = Table::new(rows, [Constraint::Length(11); 2]);
		Widget::render(table, Rect::new(0, 0, 23, 3), &mut buf);
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "Cell1       Cell2      ",
                "Cell3-Line1 Cell4-Line1",
                "Cell3-Line2 Cell4-Line2",
            ]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_alignment() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
		let rows = vec![
			Row::new(vec![Line::from("Left").alignment(Alignment::Left)]),
			Row::new(vec![Line::from("Center").alignment(Alignment::Center)]),
			Row::new(vec![Line::from("Right").alignment(Alignment::Right)]),
		];
		let table = Table::new(rows, [Percentage(100)]);
		Widget::render(table, Rect::new(0, 0, 10, 3), &mut buf);
		let expected = Buffer::with_lines(["Left      ", "  Center  ", "     Right"]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_overflow_does_not_panic() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
		let table = Table::new(Vec::<Row>::new(), [Constraint::Min(20); 1])
			.header(Row::new([Line::from("").alignment(Alignment::Right)]))
			.footer(Row::new([Line::from("").alignment(Alignment::Right)]));
		Widget::render(table, Rect::new(0, 0, 20, 3), &mut buf);
	}

	#[test]
	fn render_with_selected_column_and_incorrect_width_count_does_not_panic() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
		let table = Table::new(
			vec![Row::new(vec!["Row1", "Row2", "Row3"])],
			[Constraint::Length(10); 1],
		);
		let mut state = TableState::new().with_selected_column(2);
		StatefulWidget::render(table, Rect::new(0, 0, 20, 3), &mut buf, &mut state);
	}

	#[test]
	fn render_with_selected() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 2])
			.row_highlight_style(Style::new().red())
			.highlight_symbol(">>");
		let mut state = TableState::new().with_selected(Some(0));
		StatefulWidget::render(table, Rect::new(0, 0, 15, 3), &mut buf, &mut state);
		let expected = Buffer::with_lines([
			">>Cell1 Cell2  ".red(),
			"  Cell3 Cell4  ".into(),
			"               ".into(),
		]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_selected_column() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 3));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2"]),
			Row::new(vec!["Cell3", "Cell4"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 2])
			.column_highlight_style(Style::new().blue())
			.highlight_symbol(">>");
		let mut state = TableState::new().with_selected_column(Some(1));
		StatefulWidget::render(table, Rect::new(0, 0, 15, 3), &mut buf, &mut state);
		let expected = Buffer::with_lines::<[Line; 3]>([
			Line::from(vec![
				"Cell1".into(),
				" ".into(),
				"Cell2".blue(),
				"    ".into(),
			]),
			Line::from(vec![
				"Cell3".into(),
				" ".into(),
				"Cell4".blue(),
				"    ".into(),
			]),
			Line::from(vec!["      ".into(), "     ".blue(), "    ".into()]),
		]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_selected_cell() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2", "Cell3"]),
			Row::new(vec!["Cell4", "Cell5", "Cell6"]),
			Row::new(vec!["Cell7", "Cell8", "Cell9"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 3])
			.highlight_symbol(">>")
			.cell_highlight_style(Style::new().green());
		let mut state = TableState::new().with_selected_cell((1, 2));
		StatefulWidget::render(table, Rect::new(0, 0, 20, 4), &mut buf, &mut state);
		let expected = Buffer::with_lines::<[Line; 4]>([
			Line::from(vec!["  Cell1 ".into(), "Cell2 ".into(), "Cell3".into()]),
			Line::from(vec![">>Cell4 Cell5 ".into(), "Cell6".green(), " ".into()]),
			Line::from(vec!["  Cell7 ".into(), "Cell8 ".into(), "Cell9".into()]),
			Line::from(vec!["                    ".into()]),
		]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_selected_row_and_column() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2", "Cell3"]),
			Row::new(vec!["Cell4", "Cell5", "Cell6"]),
			Row::new(vec!["Cell7", "Cell8", "Cell9"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 3])
			.highlight_symbol(">>")
			.row_highlight_style(Style::new().red())
			.column_highlight_style(Style::new().blue());
		let mut state = TableState::new().with_selected(1).with_selected_column(2);
		StatefulWidget::render(table, Rect::new(0, 0, 20, 4), &mut buf, &mut state);
		let expected = Buffer::with_lines::<[Line; 4]>([
			Line::from(vec!["  Cell1 ".into(), "Cell2 ".into(), "Cell3".blue()]),
			Line::from(vec![">>Cell4 Cell5 ".red(), "Cell6".blue(), " ".red()]),
			Line::from(vec!["  Cell7 ".into(), "Cell8 ".into(), "Cell9".blue()]),
			Line::from(vec!["              ".into(), "     ".blue(), " ".into()]),
		]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_with_selected_row_and_column_and_cell() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
		let rows = vec![
			Row::new(vec!["Cell1", "Cell2", "Cell3"]),
			Row::new(vec!["Cell4", "Cell5", "Cell6"]),
			Row::new(vec!["Cell7", "Cell8", "Cell9"]),
		];
		let table = Table::new(rows, [Constraint::Length(5); 3])
			.highlight_symbol(">>")
			.row_highlight_style(Style::new().red())
			.column_highlight_style(Style::new().blue())
			.cell_highlight_style(Style::new().green());
		let mut state = TableState::new().with_selected(1).with_selected_column(2);
		StatefulWidget::render(table, Rect::new(0, 0, 20, 4), &mut buf, &mut state);
		let expected = Buffer::with_lines::<[Line; 4]>([
			Line::from(vec!["  Cell1 ".into(), "Cell2 ".into(), "Cell3".blue()]),
			Line::from(vec![">>Cell4 Cell5 ".red(), "Cell6".green(), " ".red()]),
			Line::from(vec!["  Cell7 ".into(), "Cell8 ".into(), "Cell9".blue()]),
			Line::from(vec!["              ".into(), "     ".blue(), " ".into()]),
		]);
		assert_eq!(buf, expected);
	}

	/// Note that this includes a regression test for a bug where the table would not render the
	/// correct rows when there is no selection.
	///
	#[rstest]
	#[case::no_selection(None, 50, ["50", "51", "52", "53", "54"])]
	#[case::selection_before_offset(20, 20, ["20", "21", "22", "23", "24"])]
	#[case::selection_immediately_before_offset(49, 49, ["49", "50", "51", "52", "53"])]
	#[case::selection_at_start_of_offset(50, 50, ["50", "51", "52", "53", "54"])]
	#[case::selection_at_end_of_offset(54, 50, ["50", "51", "52", "53", "54"])]
	#[case::selection_immediately_after_offset(55, 51, ["51", "52", "53", "54", "55"])]
	#[case::selection_after_offset(80, 76, ["76", "77", "78", "79", "80"])]
	fn render_with_selection_and_offset<T: Into<Option<usize>>>(
		#[case] selected_row: T,
		#[case] expected_offset: usize,
		#[case] expected_items: [&str; 5],
	) {
		// render 100 rows offset at 50, with a selected row
		let rows = (0..100).map(|i| Row::new([i.to_string()]));
		let table = Table::new(rows, [Constraint::Length(2)]);
		let mut buf = Buffer::empty(Rect::new(0, 0, 2, 5));
		let mut state = TableState::new()
			.with_offset(50)
			.with_selected(selected_row.into());

		StatefulWidget::render(table.clone(), Rect::new(0, 0, 5, 5), &mut buf, &mut state);

		assert_eq!(buf, Buffer::with_lines(expected_items));
		assert_eq!(state.offset, expected_offset);
	}
}

// test how constraints interact with table column width allocation
mod column_widths {
	use super::*;

	#[test]
	fn length_constraint() {
		// without selection, more than needed width
		let table = Table::default().widths([Length(4), Length(4)]);
		assert_eq!(table.get_column_widths(20, 0, 0), [(0, 4), (5, 4)]);

		// with selection, more than needed width
		let table = Table::default().widths([Length(4), Length(4)]);
		assert_eq!(table.get_column_widths(20, 3, 0), [(3, 4), (8, 4)]);

		// without selection, less than needed width
		let table = Table::default().widths([Length(4), Length(4)]);
		assert_eq!(table.get_column_widths(7, 0, 0), [(0, 3), (4, 3)]);

		// with selection, less than needed width
		// <--------7px-------->
		// ┌────────┐x┌────────┐
		// │ (3, 2) │x│ (6, 1) │
		// └────────┘x└────────┘
		// column spacing (i.e. `x`) is always prioritized
		let table = Table::default().widths([Length(4), Length(4)]);
		assert_eq!(table.get_column_widths(7, 3, 0), [(3, 2), (6, 1)]);
	}

	#[test]
	fn max_constraint() {
		// without selection, more than needed width
		let table = Table::default().widths([Max(4), Max(4)]);
		assert_eq!(table.get_column_widths(20, 0, 0), [(0, 4), (5, 4)]);

		// with selection, more than needed width
		let table = Table::default().widths([Max(4), Max(4)]);
		assert_eq!(table.get_column_widths(20, 3, 0), [(3, 4), (8, 4)]);

		// without selection, less than needed width
		let table = Table::default().widths([Max(4), Max(4)]);
		assert_eq!(table.get_column_widths(7, 0, 0), [(0, 3), (4, 3)]);

		// with selection, less than needed width
		let table = Table::default().widths([Max(4), Max(4)]);
		assert_eq!(table.get_column_widths(7, 3, 0), [(3, 2), (6, 1)]);
	}

	#[test]
	fn min_constraint() {
		// in its currently stage, the "Min" constraint does not grow to use the possible
		// available length and enabling "expand_to_fill" will just stretch the last
		// constraint and not split it with all available constraints

		// without selection, more than needed width
		let table = Table::default().widths([Min(4), Min(4)]);
		assert_eq!(table.get_column_widths(20, 0, 0), [(0, 10), (11, 9)]);

		// with selection, more than needed width
		let table = Table::default().widths([Min(4), Min(4)]);
		assert_eq!(table.get_column_widths(20, 3, 0), [(3, 8), (12, 8)]);

		// without selection, less than needed width
		// allocates spacer
		let table = Table::default().widths([Min(4), Min(4)]);
		assert_eq!(table.get_column_widths(7, 0, 0), [(0, 3), (4, 3)]);

		// with selection, less than needed width
		// always allocates selection and spacer
		let table = Table::default().widths([Min(4), Min(4)]);
		assert_eq!(table.get_column_widths(7, 3, 0), [(3, 2), (6, 1)]);
	}

	#[test]
	fn percentage_constraint() {
		// without selection, more than needed width
		let table = Table::default().widths([Percentage(30), Percentage(30)]);
		assert_eq!(table.get_column_widths(20, 0, 0), [(0, 6), (7, 6)]);

		// with selection, more than needed width
		let table = Table::default().widths([Percentage(30), Percentage(30)]);
		assert_eq!(table.get_column_widths(20, 3, 0), [(3, 5), (9, 5)]);

		// without selection, less than needed width
		// rounds from positions: [0.0, 0.0, 2.1, 3.1, 5.2, 7.0]
		let table = Table::default().widths([Percentage(30), Percentage(30)]);
		assert_eq!(table.get_column_widths(7, 0, 0), [(0, 2), (3, 2)]);

		// with selection, less than needed width
		// rounds from positions: [0.0, 3.0, 5.1, 6.1, 7.0, 7.0]
		let table = Table::default().widths([Percentage(30), Percentage(30)]);
		assert_eq!(table.get_column_widths(7, 3, 0), [(3, 1), (5, 1)]);
	}

	#[test]
	fn ratio_constraint() {
		// without selection, more than needed width
		// rounds from positions: [0.00, 0.00, 6.67, 7.67, 14.33]
		let table = Table::default().widths([Ratio(1, 3), Ratio(1, 3)]);
		assert_eq!(table.get_column_widths(20, 0, 0), [(0, 7), (8, 6)]);

		// with selection, more than needed width
		// rounds from positions: [0.00, 3.00, 10.67, 17.33, 20.00]
		let table = Table::default().widths([Ratio(1, 3), Ratio(1, 3)]);
		assert_eq!(table.get_column_widths(20, 3, 0), [(3, 6), (10, 5)]);

		// without selection, less than needed width
		// rounds from positions: [0.00, 2.33, 3.33, 5.66, 7.00]
		let table = Table::default().widths([Ratio(1, 3), Ratio(1, 3)]);
		assert_eq!(table.get_column_widths(7, 0, 0), [(0, 2), (3, 3)]);

		// with selection, less than needed width
		// rounds from positions: [0.00, 3.00, 5.33, 6.33, 7.00, 7.00]
		let table = Table::default().widths([Ratio(1, 3), Ratio(1, 3)]);
		assert_eq!(table.get_column_widths(7, 3, 0), [(3, 1), (5, 2)]);
	}

	/// When more width is available than requested, the behavior is controlled by flex
	#[test]
	fn underconstrained_flex() {
		let table = Table::default().widths([Min(10), Min(10), Min(1)]);
		assert_eq!(
			table.get_column_widths(62, 0, 0),
			&[(0, 20), (21, 20), (42, 20)]
		);

		let table = Table::default()
			.widths([Min(10), Min(10), Min(1)])
			.flex(Flex::Legacy);
		assert_eq!(
			table.get_column_widths(62, 0, 0),
			&[(0, 10), (11, 10), (22, 40)]
		);

		let table = Table::default()
			.widths([Min(10), Min(10), Min(1)])
			.flex(Flex::SpaceBetween);
		assert_eq!(
			table.get_column_widths(62, 0, 0),
			&[(0, 20), (21, 20), (42, 20)]
		);
	}

	#[test]
	fn underconstrained_segment_size() {
		let table = Table::default().widths([Min(10), Min(10), Min(1)]);
		assert_eq!(
			table.get_column_widths(62, 0, 0),
			&[(0, 20), (21, 20), (42, 20)]
		);

		let table = Table::default()
			.widths([Min(10), Min(10), Min(1)])
			.flex(Flex::Legacy);
		assert_eq!(
			table.get_column_widths(62, 0, 0),
			&[(0, 10), (11, 10), (22, 40)]
		);
	}

	#[test]
	fn no_constraint_with_rows() {
		let table = Table::default()
			.rows(vec![
				Row::new(vec!["a", "b"]),
				Row::new(vec!["c", "d", "e"]),
			])
			// rows should get precedence over header
			.header(Row::new(vec!["f", "g"]))
			.footer(Row::new(vec!["h", "i"]))
			.column_spacing(0);
		assert_eq!(
			table.get_column_widths(30, 0, 3),
			&[(0, 10), (10, 10), (20, 10)]
		);
	}

	#[test]
	fn no_constraint_with_header() {
		let table = Table::default()
			.rows(vec![])
			.header(Row::new(vec!["f", "g"]))
			.column_spacing(0);
		assert_eq!(table.get_column_widths(10, 0, 2), [(0, 5), (5, 5)]);
	}

	#[test]
	fn no_constraint_with_footer() {
		let table = Table::default()
			.rows(vec![])
			.footer(Row::new(vec!["h", "i"]))
			.column_spacing(0);
		assert_eq!(table.get_column_widths(10, 0, 2), [(0, 5), (5, 5)]);
	}

	#[track_caller]
	fn test_table_with_selection<'line, Lines>(
		highlight_spacing: HighlightSpacing,
		columns: u16,
		spacing: u16,
		selection: Option<usize>,
		expected: Lines,
	) where
		Lines: IntoIterator,
		Lines::Item: Into<Line<'line>>,
	{
		let table = Table::default()
			.rows(vec![Row::new(vec!["ABCDE", "12345"])])
			.highlight_spacing(highlight_spacing)
			.highlight_symbol(">>>")
			.column_spacing(spacing);
		let area = Rect::new(0, 0, columns, 3);
		let mut buf = Buffer::empty(area);
		let mut state = TableState::default().with_selected(selection);
		StatefulWidget::render(table, area, &mut buf, &mut state);
		assert_eq!(buf, Buffer::with_lines(expected));
	}

	#[test]
	fn excess_area_highlight_symbol_and_column_spacing_allocation() {
		// no highlight_symbol rendered ever
		test_table_with_selection(
			HighlightSpacing::Never,
			15,   // width
			0,    // spacing
			None, // selection
			[
				"ABCDE  12345   ", /* default layout is Flex::Start but columns length
				                    * constraints are calculated as `max_area / n_columns`,
				                    * i.e. they are distributed amongst available space */
				"               ", // row 2
				"               ", // row 3
			],
		);

		let table = Table::default()
			.rows(vec![Row::new(vec!["ABCDE", "12345"])])
			.widths([5, 5])
			.column_spacing(0);
		let area = Rect::new(0, 0, 15, 3);
		let mut buf = Buffer::empty(area);
		Widget::render(table, area, &mut buf);
		let expected = Buffer::with_lines([
			"ABCDE12345     ", /* As reference, this is what happens when you manually
			                    * specify widths */
			"               ", // row 2
			"               ", // row 3
		]);
		assert_eq!(buf, expected);

		// no highlight_symbol rendered ever
		test_table_with_selection(
			HighlightSpacing::Never,
			15,      // width
			0,       // spacing
			Some(0), // selection
			[
				"ABCDE  12345   ", // row 1
				"               ", // row 2
				"               ", // row 3
			],
		);

		// no highlight_symbol rendered because no selection is made
		test_table_with_selection(
			HighlightSpacing::WhenSelected,
			15,   // width
			0,    // spacing
			None, // selection
			[
				"ABCDE  12345   ", // row 1
				"               ", // row 2
				"               ", // row 3
			],
		);
		// highlight_symbol rendered because selection is made
		test_table_with_selection(
			HighlightSpacing::WhenSelected,
			15,      // width
			0,       // spacing
			Some(0), // selection
			[
				">>>ABCDE 12345 ", // row 1
				"               ", // row 2
				"               ", // row 3
			],
		);

		// highlight_symbol always rendered even no selection is made
		test_table_with_selection(
			HighlightSpacing::Always,
			15,   // width
			0,    // spacing
			None, // selection
			[
				"   ABCDE 12345 ", // row 1
				"               ", // row 2
				"               ", // row 3
			],
		);

		// no highlight_symbol rendered because no selection is made
		test_table_with_selection(
			HighlightSpacing::Always,
			15,      // width
			0,       // spacing
			Some(0), // selection
			[
				">>>ABCDE 12345 ", // row 1
				"               ", // row 2
				"               ", // row 3
			],
		);
	}

	#[expect(clippy::too_many_lines)]
	#[test]
	fn insufficient_area_highlight_symbol_and_column_spacing_allocation() {
		// column spacing is prioritized over every other constraint
		test_table_with_selection(
			HighlightSpacing::Never,
			10,   // width
			1,    // spacing
			None, // selection
			[
				"ABCDE 1234", // spacing is prioritized and column is cut
				"          ", // row 2
				"          ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::WhenSelected,
			10,   // width
			1,    // spacing
			None, // selection
			[
				"ABCDE 1234", // spacing is prioritized and column is cut
				"          ", // row 2
				"          ", // row 3
			],
		);

		// this test checks that space for highlight_symbol space is always allocated.
		// this test also checks that space for column is allocated.
		//
		// Space for highlight_symbol is allocated first by splitting horizontal space
		// into highlight_symbol area and column area.
		// Then in a separate step, column widths are calculated.
		// column spacing is prioritized when column widths are calculated and last column here
		// ends up with just 1 wide
		test_table_with_selection(
			HighlightSpacing::Always,
			10,   // width
			1,    // spacing
			None, // selection
			[
				"   ABC 123", // highlight_symbol and spacing are prioritized
				"          ", // row 2
				"          ", // row 3
			],
		);

		// the following are specification tests
		test_table_with_selection(
			HighlightSpacing::Always,
			9,    // width
			1,    // spacing
			None, // selection
			[
				"   ABC 12", // highlight_symbol and spacing are prioritized
				"         ", // row 2
				"         ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::Always,
			8,    // width
			1,    // spacing
			None, // selection
			[
				"   AB 12", // highlight_symbol and spacing are prioritized
				"        ", // row 2
				"        ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::Always,
			7,    // width
			1,    // spacing
			None, // selection
			[
				"   AB 1", // highlight_symbol and spacing are prioritized
				"       ", // row 2
				"       ", // row 3
			],
		);

		let table = Table::default()
			.rows(vec![Row::new(vec!["ABCDE", "12345"])])
			.highlight_spacing(HighlightSpacing::Always)
			.flex(Flex::Legacy)
			.highlight_symbol(">>>")
			.column_spacing(1);
		let area = Rect::new(0, 0, 10, 3);
		let mut buf = Buffer::empty(area);
		Widget::render(table, area, &mut buf);
		// highlight_symbol and spacing are prioritized but columns are evenly distributed
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "   ABCDE 1",
                "          ",
                "          ",
            ]);
		assert_eq!(buf, expected);

		let table = Table::default()
			.rows(vec![Row::new(vec!["ABCDE", "12345"])])
			.highlight_spacing(HighlightSpacing::Always)
			.flex(Flex::Start)
			.highlight_symbol(">>>")
			.column_spacing(1);
		let area = Rect::new(0, 0, 10, 3);
		let mut buf = Buffer::empty(area);
		Widget::render(table, area, &mut buf);
		// highlight_symbol and spacing are prioritized but columns are evenly distributed
		#[rustfmt::skip]
            let expected = Buffer::with_lines([
                "   ABC 123",
                "          ",
                "          ",
            ]);
		assert_eq!(buf, expected);

		test_table_with_selection(
			HighlightSpacing::Never,
			10,      // width
			1,       // spacing
			Some(0), // selection
			[
				"ABCDE 1234", // spacing is prioritized
				"          ",
				"          ",
			],
		);

		test_table_with_selection(
			HighlightSpacing::WhenSelected,
			10,      // width
			1,       // spacing
			Some(0), // selection
			[
				">>>ABC 123", // row 1
				"          ", // row 2
				"          ", // row 3
			],
		);

		test_table_with_selection(
			HighlightSpacing::Always,
			10,      // width
			1,       // spacing
			Some(0), // selection
			[
				">>>ABC 123", // highlight column and spacing are prioritized
				"          ", // row 2
				"          ", // row 3
			],
		);
	}

	#[test]
	fn insufficient_area_highlight_symbol_allocation_with_no_column_spacing() {
		test_table_with_selection(
			HighlightSpacing::Never,
			10,   // width
			0,    // spacing
			None, // selection
			[
				"ABCDE12345", // row 1
				"          ", // row 2
				"          ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::WhenSelected,
			10,   // width
			0,    // spacing
			None, // selection
			[
				"ABCDE12345", // row 1
				"          ", // row 2
				"          ", // row 3
			],
		);
		// highlight symbol spacing is prioritized over all constraints
		// even if the constraints are fixed length
		// this is because highlight_symbol column is separated _before_ any of the constraint
		// widths are calculated
		test_table_with_selection(
			HighlightSpacing::Always,
			10,   // width
			0,    // spacing
			None, // selection
			[
				"   ABCD123", // highlight column and spacing are prioritized
				"          ", // row 2
				"          ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::Never,
			10,      // width
			0,       // spacing
			Some(0), // selection
			[
				"ABCDE12345", // row 1
				"          ", // row 2
				"          ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::WhenSelected,
			10,      // width
			0,       // spacing
			Some(0), // selection
			[
				">>>ABCD123", // highlight column and spacing are prioritized
				"          ", // row 2
				"          ", // row 3
			],
		);
		test_table_with_selection(
			HighlightSpacing::Always,
			10,      // width
			0,       // spacing
			Some(0), // selection
			[
				">>>ABCD123", // highlight column and spacing are prioritized
				"          ", // row 2
				"          ", // row 3
			],
		);
	}
}

#[test]
fn stylize() {
	assert_eq!(
		Table::new(vec![Row::new(vec![Cell::from("")])], [Percentage(100)])
			.black()
			.on_white()
			.bold()
			.not_crossed_out()
			.style,
		Style::default()
			.fg(Color::Black)
			.bg(Color::White)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::CROSSED_OUT)
	);
}

#[rstest]
#[case::no_columns(vec![], vec![], vec![], 0)]
#[case::only_header(vec!["H1", "H2"], vec![], vec![], 2)]
#[case::only_rows(
        vec![],
        vec![vec!["C1", "C2"], vec!["C1", "C2", "C3"]],
        vec![],
        3
    )]
#[case::only_footer(vec![], vec![], vec!["F1", "F2", "F3", "F4"], 4)]
#[case::rows_longer(
        vec!["H1", "H2", "H3", "H4"],
        vec![vec!["C1", "C2"],vec!["C1", "C2", "C3"]],
        vec!["F1", "F2"],
        4
    )]
#[case::rows_longer(
        vec!["H1", "H2"],
        vec![vec!["C1", "C2"], vec!["C1", "C2", "C3", "C4"]],
        vec!["F1", "F2"],
        4
    )]
#[case::footer_longer(
        vec!["H1", "H2"],
        vec![vec!["C1", "C2"], vec!["C1", "C2", "C3"]],
        vec!["F1", "F2", "F3", "F4"],
        4
    )]

fn column_count(
	#[case] header: Vec<&str>,
	#[case] rows: Vec<Vec<&str>>,
	#[case] footer: Vec<&str>,
	#[case] expected: usize,
) {
	let header = Row::new(header);
	let footer = Row::new(footer);
	let rows: Vec<Row> = rows.into_iter().map(Row::new).collect();
	let table = Table::new(rows, Vec::<Constraint>::new())
		.header(header)
		.footer(footer);
	let column_count = table.column_count();
	assert_eq!(column_count, expected);
}

#[test]
fn render_in_minimal_buffer() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
	let rows = vec![
		Row::new(vec!["Cell1", "Cell2", "Cell3"]),
		Row::new(vec!["Cell4", "Cell5", "Cell6"]),
	];
	let table = Table::new(rows, [Constraint::Length(10); 3])
		.header(Row::new(vec!["Header1", "Header2", "Header3"]))
		.footer(Row::new(vec!["Footer1", "Footer2", "Footer3"]));
	// This should not panic, even if the buffer is too small to render the table.
	Widget::render(table, buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines([" "]));
}

#[test]
fn render_in_zero_size_buffer() {
	let mut buffer = Buffer::empty(Rect::ZERO);
	let rows = vec![
		Row::new(vec!["Cell1", "Cell2", "Cell3"]),
		Row::new(vec!["Cell4", "Cell5", "Cell6"]),
	];
	let table = Table::new(rows, [Constraint::Length(10); 3])
		.header(Row::new(vec!["Header1", "Header2", "Header3"]))
		.footer(Row::new(vec!["Footer1", "Footer2", "Footer3"]));
	// This should not panic, even if the buffer has zero size.
	Widget::render(table, buffer.area, &mut buffer);
}
