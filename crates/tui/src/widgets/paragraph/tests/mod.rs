use alloc::vec;

use rstest::rstest;

use super::*;
use crate::buffer::Buffer;
use crate::layout::{HorizontalAlignment, Rect};
use crate::style::{Color, Modifier, Style, Stylize};
use crate::text::{Line, Span, Text};
use crate::widgets::Widget;
use crate::widgets::block::TitlePosition;
use crate::widgets::borders::{BorderType, Borders};

mod alignment;
mod edge_cases;
mod metrics;
mod rendering;
mod styling;
mod wrapping;

/// Tests the [`Paragraph`] widget against the expected [`Buffer`] by rendering it onto an equal
/// area and comparing the rendered and expected content.
/// This can be used for easy testing of varying configured paragraphs with the same expected
/// buffer or any other test case really.
#[track_caller]
pub(super) fn test_case(paragraph: &Paragraph, expected: &Buffer) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, expected.area.width, expected.area.height));
	paragraph.render(buffer.area, &mut buffer);
	assert_eq!(buffer, *expected);
}

#[test]
fn can_be_stylized() {
	assert_eq!(
		Paragraph::new("").black().on_white().bold().not_dim().style,
		Style::default()
			.fg(Color::Black)
			.bg(Color::White)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::DIM)
	);
}

#[test]
fn left_aligned() {
	let p = Paragraph::new("Hello, world!").left_aligned();
	assert_eq!(p.alignment, HorizontalAlignment::Left);
}

#[test]
fn centered() {
	let p = Paragraph::new("Hello, world!").centered();
	assert_eq!(p.alignment, HorizontalAlignment::Center);
}

#[test]
fn right_aligned() {
	let p = Paragraph::new("Hello, world!").right_aligned();
	assert_eq!(p.alignment, HorizontalAlignment::Right);
}
