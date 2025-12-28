use alloc::format;
use alloc::string::ToString;
use core::iter;
use std::{dbg, println};

use itertools::Itertools;
use rstest::{fixture, rstest};

use super::*;
use crate::style::{Color, Modifier, Stylize};

#[test]
fn debug_empty_buffer() {
	let buffer = Buffer::empty(Rect::ZERO);
	let result = format!("{buffer:?}");
	println!("{result}");
	let expected = "Buffer {\n    area: Rect { x: 0, y: 0, width: 0, height: 0 }\n}";
	assert_eq!(result, expected);
}

#[cfg(feature = "underline-color")]
#[test]
fn debug_grapheme_override() {
	let buffer = Buffer::with_lines(["a🦀b"]);
	let result = format!("{buffer:?}");
	println!("{result}");
	let expected = indoc::indoc!(
		r#"
            Buffer {
                area: Rect { x: 0, y: 0, width: 4, height: 1 },
                content: [
                    "a🦀b", // hidden by multi-width symbols: [(2, " ")]
                ],
                styles: [
                    x: 0, y: 0, fg: Reset, bg: Reset, underline: Reset, modifier: NONE,
                ]
            }"#
	);
	assert_eq!(result, expected);
}

#[test]
fn debug_some_example() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 2));
	buffer.set_string(0, 0, "Hello World!", Style::default());
	buffer.set_string(
		0,
		1,
		"G'day World!",
		Style::default()
			.fg(Color::Green)
			.bg(Color::Yellow)
			.add_modifier(Modifier::BOLD),
	);
	let result = format!("{buffer:?}");
	println!("{result}");
	#[cfg(feature = "underline-color")]
	let expected = indoc::indoc!(
		r#"
            Buffer {
                area: Rect { x: 0, y: 0, width: 12, height: 2 },
                content: [
                    "Hello World!",
                    "G'day World!",
                ],
                styles: [
                    x: 0, y: 0, fg: Reset, bg: Reset, underline: Reset, modifier: NONE,
                    x: 0, y: 1, fg: Green, bg: Yellow, underline: Reset, modifier: BOLD,
                ]
            }"#
	);
	#[cfg(not(feature = "underline-color"))]
	let expected = indoc::indoc!(
		r#"
            Buffer {
                area: Rect { x: 0, y: 0, width: 12, height: 2 },
                content: [
                    "Hello World!",
                    "G'day World!",
                ],
                styles: [
                    x: 0, y: 0, fg: Reset, bg: Reset, modifier: NONE,
                    x: 0, y: 1, fg: Green, bg: Yellow, modifier: BOLD,
                ]
            }"#
	);

	assert_eq!(result, expected);
}

#[test]
fn it_translates_to_and_from_coordinates() {
	let rect = Rect::new(200, 100, 50, 80);
	let buf = Buffer::empty(rect);

	// First cell is at the upper left corner.
	assert_eq!(buf.pos_of(0), (200, 100));
	assert_eq!(buf.index_of(200, 100), 0);

	// Last cell is in the lower right.
	assert_eq!(buf.pos_of(buf.content.len() - 1), (249, 179));
	assert_eq!(buf.index_of(249, 179), buf.content.len() - 1);
}

#[test]
#[should_panic(expected = "outside the buffer")]
fn pos_of_panics_on_out_of_bounds() {
	let rect = Rect::new(0, 0, 10, 10);
	let buf = Buffer::empty(rect);

	// There are a total of 100 cells; zero-indexed means that 100 would be the 101st cell.
	let _ = buf.pos_of(100);
}

#[rstest]
#[case::left(9, 10)]
#[case::top(10, 9)]
#[case::right(20, 10)]
#[case::bottom(10, 20)]
#[should_panic(
	expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is"
)]
fn index_of_panics_on_out_of_bounds(#[case] x: u16, #[case] y: u16) {
	let _ = Buffer::empty(Rect::new(10, 10, 10, 10)).index_of(x, y);
}

#[test]
fn test_cell() {
	let buf = Buffer::with_lines(["Hello", "World"]);

	let mut expected = Cell::default();
	expected.set_symbol("H");

	assert_eq!(buf.cell((0, 0)), Some(&expected));
	assert_eq!(buf.cell((10, 10)), None);
	assert_eq!(buf.cell(Position::new(0, 0)), Some(&expected));
	assert_eq!(buf.cell(Position::new(10, 10)), None);
}

#[test]
fn test_cell_mut() {
	let mut buf = Buffer::with_lines(["Hello", "World"]);

	let mut expected = Cell::default();
	expected.set_symbol("H");

	assert_eq!(buf.cell_mut((0, 0)), Some(&mut expected));
	assert_eq!(buf.cell_mut((10, 10)), None);
	assert_eq!(buf.cell_mut(Position::new(0, 0)), Some(&mut expected));
	assert_eq!(buf.cell_mut(Position::new(10, 10)), None);
}

#[test]
fn index() {
	let buf = Buffer::with_lines(["Hello", "World"]);

	let mut expected = Cell::default();
	expected.set_symbol("H");

	assert_eq!(buf[(0, 0)], expected);
}

#[rstest]
#[case::left(9, 10)]
#[case::top(10, 9)]
#[case::right(20, 10)]
#[case::bottom(10, 20)]
#[should_panic(
	expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is"
)]
fn index_out_of_bounds_panics(#[case] x: u16, #[case] y: u16) {
	let rect = Rect::new(10, 10, 10, 10);
	let buf = Buffer::empty(rect);
	let _ = buf[(x, y)];
}

#[test]
fn index_mut() {
	let mut buf = Buffer::with_lines(["Cat", "Dog"]);
	buf[(0, 0)].set_symbol("B");
	buf[Position::new(0, 1)].set_symbol("L");
	assert_eq!(buf, Buffer::with_lines(["Bat", "Log"]));
}

#[rstest]
#[case::left(9, 10)]
#[case::top(10, 9)]
#[case::right(20, 10)]
#[case::bottom(10, 20)]
#[should_panic(
	expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is"
)]
fn index_mut_out_of_bounds_panics(#[case] x: u16, #[case] y: u16) {
	let mut buf = Buffer::empty(Rect::new(10, 10, 10, 10));
	buf[(x, y)].set_symbol("A");
}

#[test]
fn set_string() {
	let area = Rect::new(0, 0, 5, 1);
	let mut buffer = Buffer::empty(area);

	// Zero-width
	buffer.set_stringn(0, 0, "aaa", 0, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["     "]));

	buffer.set_string(0, 0, "aaa", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["aaa  "]));

	// Width limit:
	buffer.set_stringn(0, 0, "bbbbbbbbbbbbbb", 4, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["bbbb "]));

	buffer.set_string(0, 0, "12345", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["12345"]));

	// Width truncation:
	buffer.set_string(0, 0, "123456", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["12345"]));

	// multi-line
	buffer = Buffer::empty(Rect::new(0, 0, 5, 2));
	buffer.set_string(0, 0, "12345", Style::default());
	buffer.set_string(0, 1, "67890", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["12345", "67890"]));
}

#[test]
fn set_string_multi_width_overwrite() {
	let area = Rect::new(0, 0, 5, 1);
	let mut buffer = Buffer::empty(area);

	// multi-width overwrite
	buffer.set_string(0, 0, "aaaaa", Style::default());
	buffer.set_string(0, 0, "称号", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["称号a"]));
}

#[test]
fn set_string_zero_width() {
	assert_eq!("\u{200B}".width(), 0);

	let area = Rect::new(0, 0, 1, 1);
	let mut buffer = Buffer::empty(area);

	// Leading grapheme with zero width
	let s = "\u{200B}a";
	buffer.set_stringn(0, 0, s, 1, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["a"]));

	// Trailing grapheme with zero with
	let s = "a\u{200B}";
	buffer.set_stringn(0, 0, s, 1, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["a"]));
}

#[test]
fn set_string_double_width() {
	let area = Rect::new(0, 0, 5, 1);
	let mut buffer = Buffer::empty(area);
	buffer.set_string(0, 0, "コン", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["コン "]));

	// Only 1 space left.
	buffer.set_string(0, 0, "コンピ", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["コン "]));
}

#[fixture]
fn small_one_line_buffer() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 5, 1))
}

#[rstest]
#[case::empty("", "     ")]
#[case::one("1", "1    ")]
#[case::full("12345", "12345")]
#[case::overflow("123456", "12345")]
fn set_line_raw(mut small_one_line_buffer: Buffer, #[case] content: &str, #[case] expected: &str) {
	let line = Line::raw(content);
	small_one_line_buffer.set_line(0, 0, &line, 5);

	// note: testing with empty / set_string here instead of with_lines because with_lines calls
	// set_line
	let mut expected_buffer = Buffer::empty(small_one_line_buffer.area);
	expected_buffer.set_string(0, 0, expected, Style::default());
	assert_eq!(small_one_line_buffer, expected_buffer);
}

#[rstest]
#[case::empty("", "     ")]
#[case::one("1", "1    ")]
#[case::full("12345", "12345")]
#[case::overflow("123456", "12345")]
fn set_line_styled(
	mut small_one_line_buffer: Buffer,
	#[case] content: &str,
	#[case] expected: &str,
) {
	let color = Color::Blue;
	let line = Line::styled(content, color);
	small_one_line_buffer.set_line(0, 0, &line, 5);

	// note: manually testing the contents here as the Buffer::with_lines calls set_line
	let actual_contents = small_one_line_buffer
		.content
		.iter()
		.map(Cell::symbol)
		.join("");
	let actual_styles = small_one_line_buffer
		.content
		.iter()
		.map(|c| c.fg)
		.collect_vec();

	// set_line only sets the style for non-empty cells (unlike Line::render which sets the
	// style for all cells)
	let expected_styles = iter::repeat_n(color, content.len().min(5))
		.chain(iter::repeat_n(
			Color::default(),
			5_usize.saturating_sub(content.len()),
		))
		.collect_vec();
	assert_eq!(actual_contents, expected);
	assert_eq!(actual_styles, expected_styles);
}

#[test]
fn set_style() {
	let mut buffer = Buffer::with_lines(["aaaaa", "bbbbb", "ccccc"]);
	buffer.set_style(Rect::new(0, 1, 5, 1), Style::new().red());
	#[rustfmt::skip]
        let expected = Buffer::with_lines([
            "aaaaa".into(),
            "bbbbb".red(),
            "ccccc".into(),
        ]);
	assert_eq!(buffer, expected);
}

#[test]
fn set_style_does_not_panic_when_out_of_area() {
	let mut buffer = Buffer::with_lines(["aaaaa", "bbbbb", "ccccc"]);
	buffer.set_style(Rect::new(0, 1, 10, 3), Style::new().red());
	#[rustfmt::skip]
        let expected = Buffer::with_lines([
            "aaaaa".into(),
            "bbbbb".red(),
            "ccccc".red(),
        ]);
	assert_eq!(buffer, expected);
}

#[test]
fn with_lines() {
	#[rustfmt::skip]
        let buffer = Buffer::with_lines([
            "┌────────┐",
            "│コンピュ│",
            "│ーa 上で│",
            "└────────┘",
        ]);
	assert_eq!(buffer.area.x, 0);
	assert_eq!(buffer.area.y, 0);
	assert_eq!(buffer.area.width, 10);
	assert_eq!(buffer.area.height, 4);
}

#[test]
fn diff_empty_empty() {
	let area = Rect::new(0, 0, 40, 40);
	let prev = Buffer::empty(area);
	let next = Buffer::empty(area);
	let diff = prev.diff(&next);
	assert_eq!(diff, []);
}

#[test]
fn diff_empty_filled() {
	let area = Rect::new(0, 0, 40, 40);
	let prev = Buffer::empty(area);
	let next = Buffer::filled(area, Cell::new("a"));
	let diff = prev.diff(&next);
	assert_eq!(diff.len(), 40 * 40);
}

#[test]
fn diff_filled_filled() {
	let area = Rect::new(0, 0, 40, 40);
	let prev = Buffer::filled(area, Cell::new("a"));
	let next = Buffer::filled(area, Cell::new("a"));
	let diff = prev.diff(&next);
	assert_eq!(diff, []);
}

#[test]
fn diff_single_width() {
	let prev = Buffer::with_lines([
		"          ",
		"┌Title─┐  ",
		"│      │  ",
		"│      │  ",
		"└──────┘  ",
	]);
	let next = Buffer::with_lines([
		"          ",
		"┌TITLE─┐  ",
		"│      │  ",
		"│      │  ",
		"└──────┘  ",
	]);
	let diff = prev.diff(&next);
	assert_eq!(
		diff,
		[
			(2, 1, &Cell::new("I")),
			(3, 1, &Cell::new("T")),
			(4, 1, &Cell::new("L")),
			(5, 1, &Cell::new("E")),
		]
	);
}

#[test]
fn diff_multi_width() {
	#[rustfmt::skip]
        let prev = Buffer::with_lines([
            "┌Title─┐  ",
            "└──────┘  ",
        ]);
	#[rustfmt::skip]
        let next = Buffer::with_lines([
            "┌称号──┐  ",
            "└──────┘  ",
        ]);
	let diff = prev.diff(&next);
	assert_eq!(
		diff,
		[
			(1, 0, &Cell::new("称")),
			// Skipped "i"
			(3, 0, &Cell::new("号")),
			// Skipped "l"
			(5, 0, &Cell::new("─")),
		]
	);
}

#[test]
fn diff_multi_width_offset() {
	let prev = Buffer::with_lines(["┌称号──┐"]);
	let next = Buffer::with_lines(["┌─称号─┐"]);

	let diff = prev.diff(&next);
	assert_eq!(
		diff,
		[
			(1, 0, &Cell::new("─")),
			(2, 0, &Cell::new("称")),
			(4, 0, &Cell::new("号")),
		]
	);
}

#[test]
fn diff_skip() {
	let prev = Buffer::with_lines(["123"]);
	let mut next = Buffer::with_lines(["456"]);
	for i in 1..3 {
		next.content[i].set_skip(true);
	}

	let diff = prev.diff(&next);
	assert_eq!(diff, [(0, 0, &Cell::new("4"))],);
}

#[rstest]
#[case(Rect::new(0, 0, 2, 2), Rect::new(0, 2, 2, 2), ["11", "11", "22", "22"])]
#[case(Rect::new(2, 2, 2, 2), Rect::new(0, 0, 2, 2), ["22  ", "22  ", "  11", "  11"])]
fn merge<'line, Lines>(#[case] one: Rect, #[case] two: Rect, #[case] expected: Lines)
where
	Lines: IntoIterator,
	Lines::Item: Into<Line<'line>>,
{
	let mut one = Buffer::filled(one, Cell::new("1"));
	let two = Buffer::filled(two, Cell::new("2"));
	one.merge(&two);
	assert_eq!(one, Buffer::with_lines(expected));
}

#[test]
fn merge_with_offset() {
	let mut one = Buffer::filled(
		Rect {
			x: 3,
			y: 3,
			width: 2,
			height: 2,
		},
		Cell::new("1"),
	);
	let two = Buffer::filled(
		Rect {
			x: 1,
			y: 1,
			width: 3,
			height: 4,
		},
		Cell::new("2"),
	);
	one.merge(&two);
	let mut expected = Buffer::with_lines(["222 ", "222 ", "2221", "2221"]);
	expected.area = Rect {
		x: 1,
		y: 1,
		width: 4,
		height: 4,
	};
	assert_eq!(one, expected);
}

#[rstest]
#[case(false, true, [false, false, true, true, true, true])]
#[case(true, false, [true, true, false, false, false, false])]
fn merge_skip(#[case] skip_one: bool, #[case] skip_two: bool, #[case] expected: [bool; 6]) {
	let mut one = {
		let area = Rect {
			x: 0,
			y: 0,
			width: 2,
			height: 2,
		};
		let mut cell = Cell::new("1");
		cell.skip = skip_one;
		Buffer::filled(area, cell)
	};
	let two = {
		let area = Rect {
			x: 0,
			y: 1,
			width: 2,
			height: 2,
		};
		let mut cell = Cell::new("2");
		cell.skip = skip_two;
		Buffer::filled(area, cell)
	};
	one.merge(&two);
	let skipped = one.content().iter().map(|c| c.skip).collect::<Vec<_>>();
	assert_eq!(skipped, expected);
}

#[test]
fn with_lines_accepts_into_lines() {
	use crate::style::Stylize;
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 2));
	buf.set_string(0, 0, "foo", Style::new().red());
	buf.set_string(0, 1, "bar", Style::new().blue());
	assert_eq!(buf, Buffer::with_lines(["foo".red(), "bar".blue()]));
}

#[test]
fn control_sequence_rendered_full() {
	let text = "I \x1b[0;36mwas\x1b[0m here!";

	let mut buffer = Buffer::filled(Rect::new(0, 0, 25, 3), Cell::new("x"));
	buffer.set_string(1, 1, text, Style::new());

	let expected = Buffer::with_lines([
		"xxxxxxxxxxxxxxxxxxxxxxxxx",
		"xI [0;36mwas[0m here!xxxx",
		"xxxxxxxxxxxxxxxxxxxxxxxxx",
	]);
	assert_eq!(buffer, expected);
}

#[test]
fn control_sequence_rendered_partially() {
	let text = "I \x1b[0;36mwas\x1b[0m here!";

	let mut buffer = Buffer::filled(Rect::new(0, 0, 11, 3), Cell::new("x"));
	buffer.set_string(1, 1, text, Style::new());

	#[rustfmt::skip]
        let expected = Buffer::with_lines([
            "xxxxxxxxxxx",
            "xI [0;36mwa",
            "xxxxxxxxxxx",
        ]);
	assert_eq!(buffer, expected);
}

/// Emojis normally contain various characters which should stay part of the Emoji.
/// This should work fine by utilizing `unicode_segmentation` but a testcase is probably helpful
/// due to the nature of never perfect Unicode implementations and all of its quirks.
#[rstest]
// Shrug without gender or skintone. Has a width of 2 like all emojis have.
#[case::shrug("🤷", "🤷xxxxx")]
// Technically this is a (brown) bear, a zero-width joiner and a snowflake
// As it is joined its a single emoji and should therefore have a width of 2.
// Prior to unicode-width 0.2, this was incorrectly detected as width 4 for some reason
#[case::polarbear("🐻‍❄️", "🐻‍❄️xxxxx")]
// Technically this is an eye, a zero-width joiner and a speech bubble
// Both eye and speech bubble include a 'display as emoji' variation selector
// Prior to unicode-width 0.2, this was incorrectly detected as width 4 for some reason
#[case::eye_speechbubble("👁️‍🗨️", "👁️‍🗨️xxxxx")]
// Keyboard keycap emoji: base symbol + VS16 for emoji presentation
// This should render as a single grapheme with width 2.
#[case::keyboard_emoji("⌨️", "⌨️xxxxx")]
fn renders_emoji(#[case] input: &str, #[case] expected: &str) {
	use unicode_width::UnicodeWidthChar;

	dbg!(input);
	dbg!(input.len());
	dbg!(
		input
			.graphemes(true)
			.map(|symbol| (symbol, symbol.escape_unicode().to_string(), symbol.width()))
			.collect::<Vec<_>>()
	);
	dbg!(
		input
			.chars()
			.map(|char| (
				char,
				char.escape_unicode().to_string(),
				char.width(),
				char.is_control()
			))
			.collect::<Vec<_>>()
	);

	let mut buffer = Buffer::filled(Rect::new(0, 0, 7, 1), Cell::new("x"));
	buffer.set_string(0, 0, input, Style::new());

	let expected = Buffer::with_lines([expected]);
	assert_eq!(buffer, expected);
}

/// Regression test for
///
/// Previously the `pos_of` function would incorrectly cast the index to a u16 value instead of
/// using the index as is. This caused incorrect rendering of any buffer with an length > 65535.
#[test]
fn index_pos_of_u16_max() {
	let buffer = Buffer::empty(Rect::new(0, 0, 256, 256 + 1));
	assert_eq!(buffer.index_of(255, 255), 65535);
	assert_eq!(buffer.pos_of(65535), (255, 255));

	assert_eq!(buffer.index_of(0, 256), 65536);
	assert_eq!(buffer.pos_of(65536), (0, 256)); // previously (0, 0)

	assert_eq!(buffer.index_of(1, 256), 65537);
	assert_eq!(buffer.pos_of(65537), (1, 256)); // previously (1, 0)

	assert_eq!(buffer.index_of(255, 256), 65791);
	assert_eq!(buffer.pos_of(65791), (255, 256)); // previously (255, 0)
}

#[test]
fn diff_clears_trailing_cell_for_wide_grapheme() {
	// Reproduce: write "ab", then overwrite with a wide emoji like "⌨️"
	let prev = Buffer::with_lines(["ab"]); // width 2 area inferred
	assert_eq!(prev.area.width, 2);

	let mut next = Buffer::with_lines(["  "]); // start with blanks
	next.set_string(0, 0, "⌨️", Style::new());

	// The next buffer contains a wide grapheme occupying cell 0 and implicitly cell 1.
	// The debug formatting shows the hidden trailing space.
	let expected_next = Buffer::with_lines(["⌨️"]);
	assert_eq!(next, expected_next);

	// The diff should include an update for (0,0) to draw the emoji. Depending on
	// terminal behavior, it may or may not be necessary to explicitly clear (1,0).
	// At minimum, ensure the first cell is updated and nothing incorrect is emitted.
	let diff = prev.diff(&next);
	assert!(
		diff.iter()
			.any(|(x, y, c)| *x == 0 && *y == 0 && c.symbol() == "⌨️")
	);
	// And it should explicitly clear the trailing cell (1,0) to avoid leftovers on terminals
	// that don't automatically clear the following cell for wide characters.
	assert!(
		diff.iter()
			.any(|(x, y, c)| *x == 1 && *y == 0 && c.symbol() == " ")
	);
}
