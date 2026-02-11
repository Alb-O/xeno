use rstest::fixture;

use super::*;
use crate::buffer::Cell;
use crate::layout::HorizontalAlignment;
use crate::style::Stylize;

mod conversions;
mod operators;
mod widget;

#[fixture]
pub(super) fn small_buf() -> Buffer {
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
fn reset_style() {
	let span = Span::styled("test content", Style::new().green()).reset_style();
	assert_eq!(span.style, Style::reset());
}

#[test]
fn patch_style() {
	let span = Span::styled("test content", Style::new().green().on_yellow()).patch_style(Style::new().red().bold());
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
fn left_aligned() {
	let span = Span::styled("Test Content", Style::new().green().italic());
	let line = span.into_left_aligned_line();
	assert_eq!(line.alignment, Some(HorizontalAlignment::Left));
}

#[test]
fn centered() {
	let span = Span::styled("Test Content", Style::new().green().italic());
	let line = span.into_centered_line();
	assert_eq!(line.alignment, Some(HorizontalAlignment::Center));
}

#[test]
fn right_aligned() {
	let span = Span::styled("Test Content", Style::new().green().italic());
	let line = span.into_right_aligned_line();
	assert_eq!(line.alignment, Some(HorizontalAlignment::Right));
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
		[Cell::new("H"), Cell::new("e"), Cell::new("l"), Cell::new("l"), Cell::new("o\u{200E}"),]
	);
}
