//! Buffer behavior and regression tests.

use core::iter;
use std::println;

use super::*;
use crate::style::{Color, Modifier, Stylize};

mod diff;
mod indexing;
mod merge;
mod set_content;

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
		Style::default().fg(Color::Green).bg(Color::Yellow).add_modifier(Modifier::BOLD),
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
fn with_lines_accepts_into_lines() {
	use crate::style::Stylize;
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 2));
	buf.set_string(0, 0, "foo", Style::new().red());
	buf.set_string(0, 1, "bar", Style::new().blue());
	assert_eq!(buf, Buffer::with_lines(["foo".red(), "bar".blue()]));
}
