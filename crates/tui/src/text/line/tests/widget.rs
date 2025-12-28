use std::dbg;

use rstest::{fixture, rstest};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{small_buf, *};
use crate::buffer::Cell;
use crate::style::Stylize;

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
