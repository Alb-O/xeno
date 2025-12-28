use alloc::format;
use core::iter;
use std::dbg;

use rstest::{fixture, rstest};

use super::*;
use crate::style::{Color, Modifier, Stylize};

#[fixture]
fn small_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 1))
}

#[test]
fn raw_str() {
	let line = Line::raw("test content");
	assert_eq!(line.spans, [Span::raw("test content")]);
	assert_eq!(line.alignment, None);

	let line = Line::raw("a\nb");
	assert_eq!(line.spans, [Span::raw("a"), Span::raw("b")]);
	assert_eq!(line.alignment, None);
}

#[test]
fn styled_str() {
	let style = Style::new().yellow();
	let content = "Hello, world!";
	let line = Line::styled(content, style);
	assert_eq!(line.spans, [Span::raw(content)]);
	assert_eq!(line.style, style);
}

#[test]
fn styled_string() {
	let style = Style::new().yellow();
	let content = String::from("Hello, world!");
	let line = Line::styled(content.clone(), style);
	assert_eq!(line.spans, [Span::raw(content)]);
	assert_eq!(line.style, style);
}

#[test]
fn styled_cow() {
	let style = Style::new().yellow();
	let content = Cow::from("Hello, world!");
	let line = Line::styled(content.clone(), style);
	assert_eq!(line.spans, [Span::raw(content)]);
	assert_eq!(line.style, style);
}

#[test]
fn spans_vec() {
	let line = Line::default().spans(vec!["Hello".blue(), " world!".green()]);
	assert_eq!(
		line.spans,
		vec![
			Span::styled("Hello", Style::new().blue()),
			Span::styled(" world!", Style::new().green()),
		]
	);
}

#[test]
fn spans_iter() {
	let line = Line::default().spans([1, 2, 3].iter().map(|i| format!("Item {i}")));
	assert_eq!(
		line.spans,
		vec![
			Span::raw("Item 1"),
			Span::raw("Item 2"),
			Span::raw("Item 3"),
		]
	);
}

#[test]
fn style() {
	let line = Line::default().style(Style::new().red());
	assert_eq!(line.style, Style::new().red());
}

#[test]
fn alignment() {
	let line = Line::from("This is left").alignment(Alignment::Left);
	assert_eq!(Some(Alignment::Left), line.alignment);

	let line = Line::from("This is default");
	assert_eq!(None, line.alignment);
}

#[test]
fn width() {
	let line = Line::from(vec![
		Span::styled("My", Style::default().fg(Color::Yellow)),
		Span::raw(" text"),
	]);
	assert_eq!(7, line.width());

	let empty_line = Line::default();
	assert_eq!(0, empty_line.width());
}

#[test]
fn patch_style() {
	let raw_line = Line::styled("foobar", Color::Yellow);
	let styled_line = Line::styled("foobar", (Color::Yellow, Modifier::ITALIC));

	assert_ne!(raw_line, styled_line);

	let raw_line = raw_line.patch_style(Modifier::ITALIC);
	assert_eq!(raw_line, styled_line);
}

#[test]
fn reset_style() {
	let line = Line::styled("foobar", Style::default().yellow().on_red().italic()).reset_style();

	assert_eq!(Style::reset(), line.style);
}

#[test]
fn stylize() {
	assert_eq!(Line::default().green().style, Color::Green.into());
	assert_eq!(
		Line::default().on_green().style,
		Style::new().bg(Color::Green)
	);
	assert_eq!(Line::default().italic().style, Modifier::ITALIC.into());
}

#[test]
fn from_string() {
	let s = String::from("Hello, world!");
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello, world!")]);

	let s = String::from("Hello\nworld!");
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello"), Span::from("world!")]);
}

#[test]
fn from_str() {
	let s = "Hello, world!";
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello, world!")]);

	let s = "Hello\nworld!";
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello"), Span::from("world!")]);
}

#[test]
fn to_line() {
	let line = 42.to_line();
	assert_eq!(line.spans, [Span::from("42")]);
}

#[test]
fn from_vec() {
	let spans = vec![
		Span::styled("Hello,", Style::default().fg(Color::Red)),
		Span::styled(" world!", Style::default().fg(Color::Green)),
	];
	let line = Line::from(spans.clone());
	assert_eq!(line.spans, spans);
}

#[test]
fn from_iter() {
	let line = Line::from_iter(vec!["Hello".blue(), " world!".green()]);
	assert_eq!(
		line.spans,
		vec![
			Span::styled("Hello", Style::new().blue()),
			Span::styled(" world!", Style::new().green()),
		]
	);
}

#[test]
fn collect() {
	let line: Line = iter::once("Hello".blue())
		.chain(iter::once(" world!".green()))
		.collect();
	assert_eq!(
		line.spans,
		vec![
			Span::styled("Hello", Style::new().blue()),
			Span::styled(" world!", Style::new().green()),
		]
	);
}

#[test]
fn from_span() {
	let span = Span::styled("Hello, world!", Style::default().fg(Color::Yellow));
	let line = Line::from(span.clone());
	assert_eq!(line.spans, [span]);
}

#[test]
fn add_span() {
	assert_eq!(
		Line::raw("Red").red() + Span::raw("blue").blue(),
		Line {
			spans: vec![Span::raw("Red"), Span::raw("blue").blue()],
			style: Style::new().red(),
			alignment: None,
		},
	);
}

#[test]
fn add_line() {
	assert_eq!(
		Line::raw("Red").red() + Line::raw("Blue").blue(),
		Text {
			lines: vec![Line::raw("Red").red(), Line::raw("Blue").blue()],
			style: Style::default(),
			alignment: None,
		}
	);
}

#[test]
fn add_assign_span() {
	let mut line = Line::raw("Red").red();
	line += Span::raw("Blue").blue();
	assert_eq!(
		line,
		Line {
			spans: vec![Span::raw("Red"), Span::raw("Blue").blue()],
			style: Style::new().red(),
			alignment: None,
		},
	);
}

#[test]
fn extend() {
	let mut line = Line::from("Hello, ");
	line.extend([Span::raw("world!")]);
	assert_eq!(line.spans, [Span::raw("Hello, "), Span::raw("world!")]);

	let mut line = Line::from("Hello, ");
	line.extend([Span::raw("world! "), Span::raw("How are you?")]);
	assert_eq!(
		line.spans,
		[
			Span::raw("Hello, "),
			Span::raw("world! "),
			Span::raw("How are you?")
		]
	);
}

#[test]
fn into_string() {
	let line = Line::from(vec![
		Span::styled("Hello,", Style::default().fg(Color::Red)),
		Span::styled(" world!", Style::default().fg(Color::Green)),
	]);
	let s: String = line.into();
	assert_eq!(s, "Hello, world!");
}

#[test]
fn styled_graphemes() {
	const RED: Style = Style::new().red();
	const GREEN: Style = Style::new().green();
	const BLUE: Style = Style::new().blue();
	const RED_ON_WHITE: Style = Style::new().red().on_white();
	const GREEN_ON_WHITE: Style = Style::new().green().on_white();
	const BLUE_ON_WHITE: Style = Style::new().blue().on_white();

	let line = Line::from(vec![
		Span::styled("He", RED),
		Span::styled("ll", GREEN),
		Span::styled("o!", BLUE),
	]);
	let styled_graphemes = line
		.styled_graphemes(Style::new().bg(Color::White))
		.collect::<Vec<StyledGrapheme>>();
	assert_eq!(
		styled_graphemes,
		vec![
			StyledGrapheme::new("H", RED_ON_WHITE),
			StyledGrapheme::new("e", RED_ON_WHITE),
			StyledGrapheme::new("l", GREEN_ON_WHITE),
			StyledGrapheme::new("l", GREEN_ON_WHITE),
			StyledGrapheme::new("o", BLUE_ON_WHITE),
			StyledGrapheme::new("!", BLUE_ON_WHITE),
		],
	);
}

#[test]
fn display_line_from_vec() {
	let line_from_vec = Line::from(vec![Span::raw("Hello,"), Span::raw(" world!")]);

	assert_eq!(format!("{line_from_vec}"), "Hello, world!");
}

#[test]
fn display_styled_line() {
	let styled_line = Line::styled("Hello, world!", Style::new().green().italic());

	assert_eq!(format!("{styled_line}"), "Hello, world!");
}

#[test]
fn display_line_from_styled_span() {
	let styled_span = Span::styled("Hello, world!", Style::new().green().italic());
	let line_from_styled_span = Line::from(styled_span);

	assert_eq!(format!("{line_from_styled_span}"), "Hello, world!");
}

#[test]
fn left_aligned() {
	let line = Line::from("Hello, world!").left_aligned();
	assert_eq!(line.alignment, Some(Alignment::Left));
}

#[test]
fn centered() {
	let line = Line::from("Hello, world!").centered();
	assert_eq!(line.alignment, Some(Alignment::Center));
}

#[test]
fn right_aligned() {
	let line = Line::from("Hello, world!").right_aligned();
	assert_eq!(line.alignment, Some(Alignment::Right));
}

#[test]
pub fn push_span() {
	let mut line = Line::from("A");
	line.push_span(Span::raw("B"));
	line.push_span("C");
	assert_eq!(
		line.spans,
		vec![Span::raw("A"), Span::raw("B"), Span::raw("C")]
	);
}

mod widget {
	use unicode_segmentation::UnicodeSegmentation;
	use unicode_width::UnicodeWidthStr;

	use super::*;
	use crate::buffer::Cell;

	const BLUE: Style = Style::new().blue();
	const GREEN: Style = Style::new().green();
	const ITALIC: Style = Style::new().italic();

	#[fixture]
	fn hello_world() -> Line<'static> {
		Line::from(vec![
			Span::styled("Hello ", BLUE),
			Span::styled("world!", GREEN),
		])
		.style(ITALIC)
	}

	#[test]
	fn render() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
		hello_world().render(Rect::new(0, 0, 15, 1), &mut buf);
		let mut expected = Buffer::with_lines(["Hello world!   "]);
		expected.set_style(Rect::new(0, 0, 15, 1), ITALIC);
		expected.set_style(Rect::new(0, 0, 6, 1), BLUE);
		expected.set_style(Rect::new(6, 0, 6, 1), GREEN);
		assert_eq!(buf, expected);
	}

	#[rstest]
	fn render_out_of_bounds(hello_world: Line<'static>, mut small_buf: Buffer) {
		let out_of_bounds = Rect::new(20, 20, 10, 1);
		hello_world.render(out_of_bounds, &mut small_buf);
		assert_eq!(small_buf, Buffer::empty(small_buf.area));
	}

	#[test]
	fn render_only_styles_line_area() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
		hello_world().render(Rect::new(0, 0, 15, 1), &mut buf);
		let mut expected = Buffer::with_lines(["Hello world!        "]);
		expected.set_style(Rect::new(0, 0, 15, 1), ITALIC);
		expected.set_style(Rect::new(0, 0, 6, 1), BLUE);
		expected.set_style(Rect::new(6, 0, 6, 1), GREEN);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_only_styles_first_line() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
		hello_world().render(buf.area, &mut buf);
		let mut expected = Buffer::with_lines(["Hello world!        ", "                    "]);
		expected.set_style(Rect::new(0, 0, 20, 1), ITALIC);
		expected.set_style(Rect::new(0, 0, 6, 1), BLUE);
		expected.set_style(Rect::new(6, 0, 6, 1), GREEN);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_truncates() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
		Line::from("Hello world!").render(Rect::new(0, 0, 5, 1), &mut buf);
		assert_eq!(buf, Buffer::with_lines(["Hello     "]));
	}

	#[test]
	fn render_centered() {
		let line = hello_world().alignment(Alignment::Center);
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
		line.render(Rect::new(0, 0, 15, 1), &mut buf);
		let mut expected = Buffer::with_lines([" Hello world!  "]);
		expected.set_style(Rect::new(0, 0, 15, 1), ITALIC);
		expected.set_style(Rect::new(1, 0, 6, 1), BLUE);
		expected.set_style(Rect::new(7, 0, 6, 1), GREEN);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_right_aligned() {
		let line = hello_world().alignment(Alignment::Right);
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
		line.render(Rect::new(0, 0, 15, 1), &mut buf);
		let mut expected = Buffer::with_lines(["   Hello world!"]);
		expected.set_style(Rect::new(0, 0, 15, 1), ITALIC);
		expected.set_style(Rect::new(3, 0, 6, 1), BLUE);
		expected.set_style(Rect::new(9, 0, 6, 1), GREEN);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_truncates_left() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
		Line::from("Hello world")
			.left_aligned()
			.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["Hello"]));
	}

	#[test]
	fn render_truncates_right() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
		Line::from("Hello world")
			.right_aligned()
			.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["world"]));
	}

	#[test]
	fn render_truncates_center() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
		Line::from("Hello world")
			.centered()
			.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["lo wo"]));
	}

	/// Part of a regression test for  which
	/// found panics with truncating lines that contained multi-byte characters.
	#[test]
	fn regression_1032() {
		let line = Line::from(
			"🦀 RFC8628 OAuth 2.0 Device Authorization GrantでCLIからGithubのaccess tokenを取得する",
		);
		let mut buf = Buffer::empty(Rect::new(0, 0, 83, 1));
		line.render(buf.area, &mut buf);
		assert_eq!(
			buf,
			Buffer::with_lines([
				"🦀 RFC8628 OAuth 2.0 Device Authorization GrantでCLIからGithubのaccess tokenを取得 "
			])
		);
	}

	/// Documentary test to highlight the crab emoji width / length discrepancy
	///
	/// Part of a regression test for  which
	/// found panics with truncating lines that contained multi-byte characters.
	#[test]
	fn crab_emoji_width() {
		let crab = "🦀";
		assert_eq!(crab.len(), 4); // bytes
		assert_eq!(crab.chars().count(), 1);
		assert_eq!(crab.graphemes(true).count(), 1);
		assert_eq!(crab.width(), 2); // display width
	}

	/// Part of a regression test for  which
	/// found panics with truncating lines that contained multi-byte characters.
	#[rstest]
	#[case::left_4(Alignment::Left, 4, "1234")]
	#[case::left_5(Alignment::Left, 5, "1234 ")]
	#[case::left_6(Alignment::Left, 6, "1234🦀")]
	#[case::left_7(Alignment::Left, 7, "1234🦀7")]
	#[case::right_4(Alignment::Right, 4, "7890")]
	#[case::right_5(Alignment::Right, 5, " 7890")]
	#[case::right_6(Alignment::Right, 6, "🦀7890")]
	#[case::right_7(Alignment::Right, 7, "4🦀7890")]
	fn render_truncates_emoji(
		#[case] alignment: Alignment,
		#[case] buf_width: u16,
		#[case] expected: &str,
	) {
		let line = Line::from("1234🦀7890").alignment(alignment);
		let mut buf = Buffer::empty(Rect::new(0, 0, buf_width, 1));
		line.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	/// Part of a regression test for  which
	/// found panics with truncating lines that contained multi-byte characters.
	///
	/// centering is tricky because there's an ambiguity about whether to take one more char
	/// from the left or the right when the line width is odd. This interacts with the width of
	/// the crab emoji, which is 2 characters wide by hitting the left or right side of the
	/// emoji.
	#[rstest]
	#[case::center_6_0(6, 0, "")]
	#[case::center_6_1(6, 1, " ")] // lef side of "🦀"
	#[case::center_6_2(6, 2, "🦀")]
	#[case::center_6_3(6, 3, "b🦀")]
	#[case::center_6_4(6, 4, "b🦀c")]
	#[case::center_7_0(7, 0, "")]
	#[case::center_7_1(7, 1, " ")] // right side of "🦀"
	#[case::center_7_2(7, 2, "🦀")]
	#[case::center_7_3(7, 3, "🦀c")]
	#[case::center_7_4(7, 4, "b🦀c")]
	#[case::center_8_0(8, 0, "")]
	#[case::center_8_1(8, 1, " ")] // right side of "🦀"
	#[case::center_8_2(8, 2, " c")] // right side of "🦀c"
	#[case::center_8_3(8, 3, "🦀c")]
	#[case::center_8_4(8, 4, "🦀cd")]
	#[case::center_8_5(8, 5, "b🦀cd")]
	#[case::center_9_0(9, 0, "")]
	#[case::center_9_1(9, 1, "c")]
	#[case::center_9_2(9, 2, " c")] // right side of "🦀c"
	#[case::center_9_3(9, 3, " cd")]
	#[case::center_9_4(9, 4, "🦀cd")]
	#[case::center_9_5(9, 5, "🦀cde")]
	#[case::center_9_6(9, 6, "b🦀cde")]
	fn render_truncates_emoji_center(
		#[case] line_width: u16,
		#[case] buf_width: u16,
		#[case] expected: &str,
	) {
		// because the crab emoji is 2 characters wide, it will can cause the centering tests
		// intersect with either the left or right part of the emoji, which causes the emoji to
		// be not rendered. Checking for four different widths of the line is enough to cover
		// all the possible cases.
		let value = match line_width {
			6 => "ab🦀cd",
			7 => "ab🦀cde",
			8 => "ab🦀cdef",
			9 => "ab🦀cdefg",
			_ => unreachable!(),
		};
		let line = Line::from(value).centered();
		let mut buf = Buffer::empty(Rect::new(0, 0, buf_width, 1));
		line.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	/// Ensures the rendering also works away from the 0x0 position.
	///
	/// Particularly of note is that an emoji that is truncated will not overwrite the
	/// characters that are already in the buffer. This is inentional (consider how a line
	/// that is rendered on a border should not overwrite the border with a partial emoji).
	#[rstest]
	#[case::left(Alignment::Left, "XXa🦀bcXXX")]
	#[case::center(Alignment::Center, "XX🦀bc🦀XX")]
	#[case::right(Alignment::Right, "XXXbc🦀dXX")]
	fn render_truncates_away_from_0x0(#[case] alignment: Alignment, #[case] expected: &str) {
		let line = Line::from(vec![Span::raw("a🦀b"), Span::raw("c🦀d")]).alignment(alignment);
		// Fill buffer with stuff to ensure the output is indeed padded
		let mut buf = Buffer::filled(Rect::new(0, 0, 10, 1), Cell::new("X"));
		let area = Rect::new(2, 0, 6, 1);
		line.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	/// When two spans are rendered after each other the first needs to be padded in accordance
	/// to the skipped unicode width. In this case the first crab does not fit at width 6 which
	/// takes a front white space.
	#[rstest]
	#[case::right_4(4, "c🦀d")]
	#[case::right_5(5, "bc🦀d")]
	#[case::right_6(6, "Xbc🦀d")]
	#[case::right_7(7, "🦀bc🦀d")]
	#[case::right_8(8, "a🦀bc🦀d")]
	fn render_right_aligned_multi_span(#[case] buf_width: u16, #[case] expected: &str) {
		let line = Line::from(vec![Span::raw("a🦀b"), Span::raw("c🦀d")]).right_aligned();
		let area = Rect::new(0, 0, buf_width, 1);
		// Fill buffer with stuff to ensure the output is indeed padded
		let mut buf = Buffer::filled(area, Cell::new("X"));
		line.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	/// Part of a regression test for  which
	/// found panics with truncating lines that contained multi-byte characters.
	///
	/// Flag emoji are actually two independent characters, so they can be truncated in the
	/// middle of the emoji. This test documents just the emoji part of the test.
	#[test]
	fn flag_emoji() {
		let str = "🇺🇸1234";
		assert_eq!(str.len(), 12); // flag is 4 bytes
		assert_eq!(str.chars().count(), 6); // flag is 2 chars
		assert_eq!(str.graphemes(true).count(), 5); // flag is 1 grapheme
		assert_eq!(str.width(), 6); // flag is 2 display width
	}

	/// Part of a regression test for  which
	/// found panics with truncating lines that contained multi-byte characters.
	#[rstest]
	#[case::flag_1(1, " ")]
	#[case::flag_2(2, "🇺🇸")]
	#[case::flag_3(3, "🇺🇸1")]
	#[case::flag_4(4, "🇺🇸12")]
	#[case::flag_5(5, "🇺🇸123")]
	#[case::flag_6(6, "🇺🇸1234")]
	#[case::flag_7(7, "🇺🇸1234 ")]
	fn render_truncates_flag(#[case] buf_width: u16, #[case] expected: &str) {
		let line = Line::from("🇺🇸1234");
		let mut buf = Buffer::empty(Rect::new(0, 0, buf_width, 1));
		line.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	// Buffer width is `u16`. A line can be longer.
	#[rstest]
	#[case::left(Alignment::Left, "This is some content with a some")]
	#[case::right(Alignment::Right, "horribly long Line over u16::MAX")]
	fn render_truncates_very_long_line_of_many_spans(
		#[case] alignment: Alignment,
		#[case] expected: &str,
	) {
		let part = "This is some content with a somewhat long width to be repeated over and over again to create horribly long Line over u16::MAX";
		let min_width = usize::from(u16::MAX).saturating_add(1);

		// width == len as only ASCII is used here
		let factor = min_width.div_ceil(part.len());

		let line = Line::from(vec![Span::raw(part); factor]).alignment(alignment);

		dbg!(line.width());
		assert!(line.width() >= min_width);

		let mut buf = Buffer::empty(Rect::new(0, 0, 32, 1));
		line.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	// Buffer width is `u16`. A single span inside a line can be longer.
	#[rstest]
	#[case::left(Alignment::Left, "This is some content with a some")]
	#[case::right(Alignment::Right, "horribly long Line over u16::MAX")]
	fn render_truncates_very_long_single_span_line(
		#[case] alignment: Alignment,
		#[case] expected: &str,
	) {
		let part = "This is some content with a somewhat long width to be repeated over and over again to create horribly long Line over u16::MAX";
		let min_width = usize::from(u16::MAX).saturating_add(1);

		// width == len as only ASCII is used here
		let factor = min_width.div_ceil(part.len());

		let line = Line::from(vec![Span::raw(part.repeat(factor))]).alignment(alignment);

		dbg!(line.width());
		assert!(line.width() >= min_width);

		let mut buf = Buffer::empty(Rect::new(0, 0, 32, 1));
		line.render(buf.area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([expected]));
	}

	#[test]
	fn render_with_newlines() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 11, 1));
		Line::from("Hello\nworld!").render(Rect::new(0, 0, 11, 1), &mut buf);
		assert_eq!(buf, Buffer::with_lines(["Helloworld!"]));
	}
}

mod iterators {
	use super::*;

	/// a fixture used in the tests below to avoid repeating the same setup
	#[fixture]
	fn hello_world() -> Line<'static> {
		Line::from(vec![
			Span::styled("Hello ", Color::Blue),
			Span::styled("world!", Color::Green),
		])
	}

	#[rstest]
	fn iter(hello_world: Line<'_>) {
		let mut iter = hello_world.iter();
		assert_eq!(iter.next(), Some(&Span::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&Span::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn iter_mut(mut hello_world: Line<'_>) {
		let mut iter = hello_world.iter_mut();
		assert_eq!(iter.next(), Some(&mut Span::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&mut Span::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn into_iter(hello_world: Line<'_>) {
		let mut iter = hello_world.into_iter();
		assert_eq!(iter.next(), Some(Span::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(Span::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn into_iter_ref(hello_world: Line<'_>) {
		let mut iter = (&hello_world).into_iter();
		assert_eq!(iter.next(), Some(&Span::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&Span::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[test]
	fn into_iter_mut_ref() {
		let mut hello_world = Line::from(vec![
			Span::styled("Hello ", Color::Blue),
			Span::styled("world!", Color::Green),
		]);
		let mut iter = (&mut hello_world).into_iter();
		assert_eq!(iter.next(), Some(&mut Span::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&mut Span::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn for_loop_ref(hello_world: Line<'_>) {
		let mut result = String::new();
		for span in &hello_world {
			result.push_str(span.content.as_ref());
		}
		assert_eq!(result, "Hello world!");
	}

	#[rstest]
	fn for_loop_mut_ref() {
		let mut hello_world = Line::from(vec![
			Span::styled("Hello ", Color::Blue),
			Span::styled("world!", Color::Green),
		]);
		let mut result = String::new();
		for span in &mut hello_world {
			result.push_str(span.content.as_ref());
		}
		assert_eq!(result, "Hello world!");
	}

	#[rstest]
	fn for_loop_into(hello_world: Line<'_>) {
		let mut result = String::new();
		for span in hello_world {
			result.push_str(span.content.as_ref());
		}
		assert_eq!(result, "Hello world!");
	}
}

#[rstest]
#[case::empty(Line::default(), "Line::default()")]
#[case::raw(Line::raw("Hello, world!"), r#"Line::from("Hello, world!")"#)]
#[case::styled(
	Line::styled("Hello, world!", Color::Yellow),
	r#"Line::from("Hello, world!").yellow()"#
)]
#[case::styled_complex(
        Line::from(String::from("Hello, world!")).green().on_blue().bold().italic().not_dim(),
        r#"Line::from("Hello, world!").green().on_blue().bold().italic().not_dim()"#
    )]
#[case::styled_span(
	Line::from(Span::styled("Hello, world!", Color::Yellow)),
	r#"Line::from(Span::from("Hello, world!").yellow())"#
)]
#[case::styled_line_and_span(
        Line::from(vec![
            Span::styled("Hello", Color::Yellow),
            Span::styled(" world!", Color::Green),
        ]).italic(),
        r#"Line::from_iter([Span::from("Hello").yellow(), Span::from(" world!").green()]).italic()"#
    )]
#[case::spans_vec(
        Line::from(vec![
            Span::styled("Hello", Color::Blue),
            Span::styled(" world!", Color::Green),
        ]),
        r#"Line::from_iter([Span::from("Hello").blue(), Span::from(" world!").green()])"#,
    )]
#[case::left_aligned(
        Line::from("Hello, world!").left_aligned(),
        r#"Line::from("Hello, world!").left_aligned()"#
    )]
#[case::centered(
        Line::from("Hello, world!").centered(),
        r#"Line::from("Hello, world!").centered()"#
    )]
#[case::right_aligned(
        Line::from("Hello, world!").right_aligned(),
        r#"Line::from("Hello, world!").right_aligned()"#
    )]
fn debug(#[case] line: Line, #[case] expected: &str) {
	assert_eq!(format!("{line:?}"), expected);
}
