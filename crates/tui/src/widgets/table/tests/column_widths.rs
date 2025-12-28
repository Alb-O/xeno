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
