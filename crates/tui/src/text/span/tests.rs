use alloc::string::String;
use alloc::{format, vec};

use rstest::{fixture, rstest};

use super::*;
use crate::buffer::Cell;
use crate::layout::Alignment;
use crate::style::Stylize;

#[fixture]
fn small_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 1))
}

#[test]
fn default() {
	let span = Span::default();
	assert_eq!(span.content, Cow::Borrowed(""));
	assert_eq!(span.style, Style::default());
}

#[test]
fn raw_str() {
	let span = Span::raw("test content");
	assert_eq!(span.content, Cow::Borrowed("test content"));
	assert_eq!(span.style, Style::default());
}

#[test]
fn raw_string() {
	let content = String::from("test content");
	let span = Span::raw(content.clone());
	assert_eq!(span.content, Cow::Owned::<str>(content));
	assert_eq!(span.style, Style::default());
}

#[test]
fn styled_str() {
	let style = Style::new().red();
	let span = Span::styled("test content", style);
	assert_eq!(span.content, Cow::Borrowed("test content"));
	assert_eq!(span.style, Style::new().red());
}

#[test]
fn styled_string() {
	let content = String::from("test content");
	let style = Style::new().green();
	let span = Span::styled(content.clone(), style);
	assert_eq!(span.content, Cow::Owned::<str>(content));
	assert_eq!(span.style, style);
}

#[test]
fn set_content() {
	let span = Span::default().content("test content");
	assert_eq!(span.content, Cow::Borrowed("test content"));
}

#[test]
fn set_style() {
	let span = Span::default().style(Style::new().green());
	assert_eq!(span.style, Style::new().green());
}

#[test]
fn from_ref_str_borrowed_cow() {
	let content = "test content";
	let span = Span::from(content);
	assert_eq!(span.content, Cow::Borrowed(content));
	assert_eq!(span.style, Style::default());
}

#[test]
fn from_string_ref_str_borrowed_cow() {
	let content = String::from("test content");
	let span = Span::from(content.as_str());
	assert_eq!(span.content, Cow::Borrowed(content.as_str()));
	assert_eq!(span.style, Style::default());
}

#[test]
fn from_string_owned_cow() {
	let content = String::from("test content");
	let span = Span::from(content.clone());
	assert_eq!(span.content, Cow::Owned::<str>(content));
	assert_eq!(span.style, Style::default());
}

#[test]
fn from_ref_string_borrowed_cow() {
	let content = String::from("test content");
	let span = Span::from(&content);
	assert_eq!(span.content, Cow::Borrowed(content.as_str()));
	assert_eq!(span.style, Style::default());
}

#[test]
fn to_span() {
	assert_eq!(42.to_span(), Span::raw("42"));
	assert_eq!("test".to_span(), Span::raw("test"));
}

#[test]
fn reset_style() {
	let span = Span::styled("test content", Style::new().green()).reset_style();
	assert_eq!(span.style, Style::reset());
}

#[test]
fn patch_style() {
	let span = Span::styled("test content", Style::new().green().on_yellow())
		.patch_style(Style::new().red().bold());
	assert_eq!(span.style, Style::new().red().on_yellow().bold());
}

#[test]
fn width() {
	assert_eq!(Span::raw("").width(), 0);
	assert_eq!(Span::raw("test").width(), 4);
	assert_eq!(Span::raw("test content").width(), 12);
	// Needs reconsideration:
	assert_eq!(Span::raw("test\ncontent").width(), 12);
}

#[test]
fn stylize() {
	let span = Span::raw("test content").green();
	assert_eq!(span.content, Cow::Borrowed("test content"));
	assert_eq!(span.style, Style::new().green());

	let span = Span::styled("test content", Style::new().green());
	let stylized = span.on_yellow().bold();
	assert_eq!(stylized.content, Cow::Borrowed("test content"));
	assert_eq!(stylized.style, Style::new().green().on_yellow().bold());
}

#[test]
fn display_span() {
	let span = Span::raw("test content");
	assert_eq!(format!("{span}"), "test content");
	assert_eq!(format!("{span:.4}"), "test");
}

#[test]
fn display_newline_span() {
	let span = Span::raw("test\ncontent");
	assert_eq!(format!("{span}"), "testcontent");
}

#[test]
fn display_styled_span() {
	let stylized_span = Span::styled("stylized test content", Style::new().green());
	assert_eq!(format!("{stylized_span}"), "stylized test content");
	assert_eq!(format!("{stylized_span:.8}"), "stylized");
}

#[test]
fn left_aligned() {
	let span = Span::styled("Test Content", Style::new().green().italic());
	let line = span.into_left_aligned_line();
	assert_eq!(line.alignment, Some(Alignment::Left));
}

#[test]
fn centered() {
	let span = Span::styled("Test Content", Style::new().green().italic());
	let line = span.into_centered_line();
	assert_eq!(line.alignment, Some(Alignment::Center));
}

#[test]
fn right_aligned() {
	let span = Span::styled("Test Content", Style::new().green().italic());
	let line = span.into_right_aligned_line();
	assert_eq!(line.alignment, Some(Alignment::Right));
}

mod widget {
	use rstest::rstest;

	use super::*;

	#[test]
	fn render() {
		let style = Style::new().green().on_yellow();
		let span = Span::styled("test content", style);
		let mut buf = Buffer::empty(Rect::new(0, 0, 15, 1));
		span.render(buf.area, &mut buf);
		let expected = Buffer::with_lines([Line::from(vec![
			"test content".green().on_yellow(),
			"   ".into(),
		])]);
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

		let expected = Buffer::with_lines([Line::from(vec![
			"test ".green().on_yellow(),
			"     ".into(),
		])]);
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
		let expected = Buffer::with_lines([Line::from(vec![
			"test content".green().on_yellow().italic(),
			"   ".italic(),
		])]);
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

		let expected =
			Buffer::with_lines([Line::from(vec!["test ".green().on_yellow(), " ".into()])]);
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

		let expected = Buffer::with_lines([Line::from(vec![
			"          ".into(),
			"test ".green().on_yellow(),
		])]);
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_first_zero_width() {
		let span = Span::raw("\u{200B}abc");
		let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
		span.render(buf.area, &mut buf);
		assert_eq!(
			buf.content(),
			[Cell::new("\u{200B}a"), Cell::new("b"), Cell::new("c"),]
		);
	}

	#[test]
	fn render_second_zero_width() {
		let span = Span::raw("a\u{200B}bc");
		let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
		span.render(buf.area, &mut buf);
		assert_eq!(
			buf.content(),
			[Cell::new("a\u{200B}"), Cell::new("b"), Cell::new("c")]
		);
	}

	#[test]
	fn render_middle_zero_width() {
		let span = Span::raw("ab\u{200B}c");
		let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
		span.render(buf.area, &mut buf);
		assert_eq!(
			buf.content(),
			[Cell::new("a"), Cell::new("b\u{200B}"), Cell::new("c")]
		);
	}

	#[test]
	fn render_last_zero_width() {
		let span = Span::raw("abc\u{200B}");
		let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
		span.render(buf.area, &mut buf);
		assert_eq!(
			buf.content(),
			[Cell::new("a"), Cell::new("b"), Cell::new("c\u{200B}")]
		);
	}

	#[test]
	fn render_with_newlines() {
		let span = Span::raw("a\nb");
		let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
		span.render(buf.area, &mut buf);
		assert_eq!(buf.content(), [Cell::new("a"), Cell::new("b")]);
	}
}

/// Regression test for  One line contains
/// some Unicode Left-Right-Marks (U+200E)
///
/// The issue was that a zero-width character at the end of the buffer causes the buffer bounds
/// to be exceeded (due to a position + 1 calculation that fails to account for the possibility
/// that the next position might not be available).
#[test]
fn issue_1160() {
	let span = Span::raw("Hello\u{200E}");
	let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
	span.render(buf.area, &mut buf);
	assert_eq!(
		buf.content(),
		[
			Cell::new("H"),
			Cell::new("e"),
			Cell::new("l"),
			Cell::new("l"),
			Cell::new("o\u{200E}"),
		]
	);
}

#[test]
fn add() {
	assert_eq!(
		Span::default() + Span::default(),
		Line::from(vec![Span::default(), Span::default()])
	);

	assert_eq!(
		Span::default() + Span::raw("test"),
		Line::from(vec![Span::default(), Span::raw("test")])
	);

	assert_eq!(
		Span::raw("test") + Span::default(),
		Line::from(vec![Span::raw("test"), Span::default()])
	);

	assert_eq!(
		Span::raw("test") + Span::raw("content"),
		Line::from(vec![Span::raw("test"), Span::raw("content")])
	);
}

#[rstest]
#[case::default(Span::default(), "Span::default()")]
#[case::raw(Span::raw("test"), r#"Span::from("test")"#)]
#[case::styled(Span::styled("test", Style::new().green()), r#"Span::from("test").green()"#)]
#[case::styled_italic(
    Span::styled("test", Style::new().green().italic()),
    r#"Span::from("test").green().italic()"#
)]
fn debug(#[case] span: Span, #[case] expected: &str) {
	assert_eq!(format!("{span:?}"), expected);
}
