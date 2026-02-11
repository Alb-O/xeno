//! Span widget rendering tests

use rstest::rstest;

use super::*;

#[test]
fn render() {
	let style = Style::new().green().on_yellow();
	let span = Span::styled("test content", style);
	let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
	span.render(buf.area, &mut buf);
	let expected = Buffer::with_lines([Line::from(vec!["test content".green().on_yellow(), "   ".into()])]);
	assert_eq!(buf, expected);
}

#[rstest]
#[case::x(20, 0)]
#[case::y(0, 20)]
#[case::both(20, 20)]
fn render_out_of_bounds(mut small_buf: Buffer, #[case] x: u16, #[case] y: u16) {
	let out_of_bounds = Rect::new(x, y, 10, 1);
	Span::raw("Hello, World!").render(out_of_bounds, &mut small_buf);
	assert_eq!(small_buf, Buffer::empty(small_buf.area));
}

/// When the content of the span is longer than the area passed to render, the content
/// should be truncated
#[test]
fn render_truncates_too_long_content() {
	let style = Style::new().green().on_yellow();
	let span = Span::styled("test content", style);

	let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
	span.render(Rect::new(0, 0, 5, 1), &mut buf);

	let expected = Buffer::with_lines([Line::from(vec!["test ".green().on_yellow(), "     ".into()])]);
	assert_eq!(buf, expected);
}

/// When there is already a style set on the buffer, the style of the span should be
/// patched with the existing style
#[test]
fn render_patches_existing_style() {
	let style = Style::new().green().on_yellow();
	let span = Span::styled("test content", style);
	let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
	buf.set_style(buf.area, Style::new().italic());
	span.render(buf.area, &mut buf);
	let expected = Buffer::with_lines([Line::from(vec!["test content".green().on_yellow().italic(), "   ".italic()])]);
	assert_eq!(buf, expected);
}

/// When the span contains a multi-width grapheme, the grapheme will ensure that the cells
/// of the hidden characters are cleared.
#[test]
fn render_multi_width_symbol() {
	let style = Style::new().green().on_yellow();
	let span = Span::styled("test 😃 content", style);
	let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
	span.render(buf.area, &mut buf);
	// The existing code in buffer.set_line() handles multi-width graphemes by clearing the
	// cells of the hidden characters. This test ensures that the existing behavior is
	// preserved.
	let expected = Buffer::with_lines(["test 😃 content".green().on_yellow()]);
	assert_eq!(buf, expected);
}

/// When the span contains a multi-width grapheme that does not fit in the area passed to
/// render, the entire grapheme will be truncated.
#[test]
fn render_multi_width_symbol_truncates_entire_symbol() {
	// the 😃 emoji is 2 columns wide so it will be truncated
	let style = Style::new().green().on_yellow();
	let span = Span::styled("test 😃 content", style);
	let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
	span.render(buf.area, &mut buf);

	let expected = Buffer::with_lines([Line::from(vec!["test ".green().on_yellow(), " ".into()])]);
	assert_eq!(buf, expected);
}

/// When the area passed to render overflows the buffer, the content should be truncated
/// to fit the buffer.
#[test]
fn render_overflowing_area_truncates() {
	let style = Style::new().green().on_yellow();
	let span = Span::styled("test content", style);
	let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
	span.render(Rect::new(10, 0, 20, 1), &mut buf);

	let expected = Buffer::with_lines([Line::from(vec!["          ".into(), "test ".green().on_yellow()])]);
	assert_eq!(buf, expected);
}

#[test]
fn render_first_zero_width() {
	let span = Span::raw("\u{200B}abc");
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
	span.render(buf.area, &mut buf);
	assert_eq!(buf.content(), [Cell::new("\u{200B}a"), Cell::new("b"), Cell::new("c"),]);
}

#[test]
fn render_second_zero_width() {
	let span = Span::raw("a\u{200B}bc");
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
	span.render(buf.area, &mut buf);
	assert_eq!(buf.content(), [Cell::new("a\u{200B}"), Cell::new("b"), Cell::new("c")]);
}

#[test]
fn render_middle_zero_width() {
	let span = Span::raw("ab\u{200B}c");
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
	span.render(buf.area, &mut buf);
	assert_eq!(buf.content(), [Cell::new("a"), Cell::new("b\u{200B}"), Cell::new("c")]);
}

#[test]
fn render_last_zero_width() {
	let span = Span::raw("abc\u{200B}");
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
	span.render(buf.area, &mut buf);
	assert_eq!(buf.content(), [Cell::new("a"), Cell::new("b"), Cell::new("c\u{200B}")]);
}

#[test]
fn render_with_newlines() {
	let span = Span::raw("a\nb");
	let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
	span.render(buf.area, &mut buf);
	assert_eq!(buf.content(), [Cell::new("a"), Cell::new("b")]);
}
