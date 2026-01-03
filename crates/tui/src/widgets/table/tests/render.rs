use super::*;
use crate::layout::HorizontalAlignment;

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
		Row::new(vec![
			Line::from("Left").alignment(HorizontalAlignment::Left),
		]),
		Row::new(vec![
			Line::from("Center").alignment(HorizontalAlignment::Center),
		]),
		Row::new(vec![
			Line::from("Right").alignment(HorizontalAlignment::Right),
		]),
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
		.header(Row::new([
			Line::from("").alignment(HorizontalAlignment::Right)
		]))
		.footer(Row::new([
			Line::from("").alignment(HorizontalAlignment::Right)
		]));
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
