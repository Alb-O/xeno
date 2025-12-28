//! Tests for the Text type.

use alloc::format;
use core::iter;

use rstest::{fixture, rstest};

use super::*;
use crate::style::{Color, Modifier, Stylize};

mod conversions;
mod debug;
mod iterators;
mod operators;
mod widget;

#[fixture]
pub(super) fn small_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 1))
}

#[test]
fn raw() {
	let text = Text::raw("The first line\nThe second line");
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn styled() {
	let style = Style::new().yellow().italic();
	let styled_text = Text::styled("The first line\nThe second line", style);

	let mut text = Text::raw("The first line\nThe second line");
	text.style = style;

	assert_eq!(styled_text, text);
}

#[test]
fn width() {
	let text = Text::from("The first line\nThe second line");
	assert_eq!(15, text.width());
}

#[test]
fn height() {
	let text = Text::from("The first line\nThe second line");
	assert_eq!(2, text.height());
}

#[test]
fn patch_style() {
	let style = Style::new().yellow().italic();
	let style2 = Style::new().red().underlined();
	let text = Text::styled("The first line\nThe second line", style).patch_style(style2);

	let expected_style = Style::new().red().italic().underlined();
	let expected_text = Text::styled("The first line\nThe second line", expected_style);

	assert_eq!(text, expected_text);
}

#[test]
fn reset_style() {
	let style = Style::new().yellow().italic();
	let text = Text::styled("The first line\nThe second line", style).reset_style();

	assert_eq!(text.style, Style::reset());
}

#[test]
fn stylize() {
	assert_eq!(Text::default().green().style, Color::Green.into());
	assert_eq!(
		Text::default().on_green().style,
		Style::new().bg(Color::Green)
	);
	assert_eq!(Text::default().italic().style, Modifier::ITALIC.into());
}

#[test]
fn left_aligned() {
	let text = Text::from("Hello, world!").left_aligned();
	assert_eq!(text.alignment, Some(Alignment::Left));
}

#[test]
fn centered() {
	let text = Text::from("Hello, world!").centered();
	assert_eq!(text.alignment, Some(Alignment::Center));
}

#[test]
fn right_aligned() {
	let text = Text::from("Hello, world!").right_aligned();
	assert_eq!(text.alignment, Some(Alignment::Right));
}

#[test]
fn push_line() {
	let mut text = Text::from("A");
	text.push_line(Line::from("B"));
	text.push_line(Span::from("C"));
	text.push_line("D");
	assert_eq!(
		text.lines,
		vec![
			Line::raw("A"),
			Line::raw("B"),
			Line::raw("C"),
			Line::raw("D")
		]
	);
}

#[test]
fn push_line_empty() {
	let mut text = Text::default();
	text.push_line(Line::from("Hello, world!"));
	assert_eq!(text.lines, [Line::from("Hello, world!")]);
}

#[test]
fn push_span() {
	let mut text = Text::from("A");
	text.push_span(Span::raw("B"));
	text.push_span("C");
	assert_eq!(
		text.lines,
		vec![Line::from(vec![
			Span::raw("A"),
			Span::raw("B"),
			Span::raw("C")
		])],
	);
}

#[test]
fn push_span_empty() {
	let mut text = Text::default();
	text.push_span(Span::raw("Hello, world!"));
	assert_eq!(text.lines, [Line::from(Span::raw("Hello, world!"))]);
}
