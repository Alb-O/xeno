use alloc::vec;

use rstest::rstest;

use super::*;
use crate::buffer::Buffer;
use crate::layout::{Alignment, Rect};
use crate::style::{Color, Modifier, Style, Stylize};
use crate::text::{Line, Span, Text};
use crate::widgets::Widget;
use crate::widgets::block::TitlePosition;
use crate::widgets::borders::{BorderType, Borders};

/// Tests the [`Paragraph`] widget against the expected [`Buffer`] by rendering it onto an equal
/// area and comparing the rendered and expected content.
/// This can be used for easy testing of varying configured paragraphs with the same expected
/// buffer or any other test case really.
#[track_caller]
fn test_case(paragraph: &Paragraph, expected: &Buffer) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, expected.area.width, expected.area.height));
	paragraph.render(buffer.area, &mut buffer);
	assert_eq!(buffer, *expected);
}

#[test]
fn zero_width_char_at_end_of_line() {
	let line = "foo\u{200B}";
	for paragraph in [
		Paragraph::new(line),
		Paragraph::new(line).wrap(Wrap { trim: false }),
		Paragraph::new(line).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::with_lines(["foo"]));
		test_case(&paragraph, &Buffer::with_lines(["foo   "]));
		test_case(&paragraph, &Buffer::with_lines(["foo   ", "      "]));
		test_case(&paragraph, &Buffer::with_lines(["foo", "   "]));
	}
}

#[test]
fn test_render_empty_paragraph() {
	for paragraph in [
		Paragraph::new(""),
		Paragraph::new("").wrap(Wrap { trim: false }),
		Paragraph::new("").wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::with_lines([" "]));
		test_case(&paragraph, &Buffer::with_lines(["          "]));
		test_case(&paragraph, &Buffer::with_lines(["     "; 10]));
		test_case(&paragraph, &Buffer::with_lines([" ", " "]));
	}
}

#[test]
fn test_render_single_line_paragraph() {
	let text = "Hello, world!";
	for paragraph in [
		Paragraph::new(text),
		Paragraph::new(text).wrap(Wrap { trim: false }),
		Paragraph::new(text).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::with_lines(["Hello, world!  "]));
		test_case(&paragraph, &Buffer::with_lines(["Hello, world!"]));
		test_case(
			&paragraph,
			&Buffer::with_lines(["Hello, world!  ", "               "]),
		);
		test_case(
			&paragraph,
			&Buffer::with_lines(["Hello, world!", "             "]),
		);
	}
}

#[test]
fn test_render_multi_line_paragraph() {
	let text = "This is a\nmultiline\nparagraph.";
	for paragraph in [
		Paragraph::new(text),
		Paragraph::new(text).wrap(Wrap { trim: false }),
		Paragraph::new(text).wrap(Wrap { trim: true }),
	] {
		test_case(
			&paragraph,
			&Buffer::with_lines(["This is a ", "multiline ", "paragraph."]),
		);
		test_case(
			&paragraph,
			&Buffer::with_lines(["This is a      ", "multiline      ", "paragraph.     "]),
		);
		test_case(
			&paragraph,
			&Buffer::with_lines([
				"This is a      ",
				"multiline      ",
				"paragraph.     ",
				"               ",
				"               ",
			]),
		);
	}
}

#[test]
fn test_render_paragraph_with_block() {
	// We use the slightly unconventional "worlds" instead of "world" here to make sure when we
	// can truncate this without triggering the typos linter.
	let text = "Hello, worlds!";
	let truncated_paragraph = Paragraph::new(text).block(
		Block::bordered()
			.border_type(BorderType::Plain)
			.title("Title"),
	);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		#[rustfmt::skip]
            test_case(
                paragraph,
                &Buffer::with_lines([
                    "┌Title─────────┐",
                    "│Hello, worlds!│",
                    "└──────────────┘",
                ]),
            );
		test_case(
			paragraph,
			&Buffer::with_lines([
				"┌Title───────────┐",
				"│Hello, worlds!  │",
				"└────────────────┘",
			]),
		);
		test_case(
			paragraph,
			&Buffer::with_lines([
				"┌Title────────────┐",
				"│Hello, worlds!   │",
				"│                 │",
				"└─────────────────┘",
			]),
		);
	}

	test_case(
		&truncated_paragraph,
		&Buffer::with_lines([
			"┌Title───────┐",
			"│Hello, world│",
			"│            │",
			"└────────────┘",
		]),
	);
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines([
			"┌Title──────┐",
			"│Hello,     │",
			"│worlds!    │",
			"└───────────┘",
		]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines([
			"┌Title──────┐",
			"│Hello,     │",
			"│worlds!    │",
			"└───────────┘",
		]),
	);
}

#[test]
fn test_render_line_styled() {
	let l0 = Line::raw("unformatted");
	let l1 = Line::styled("bold text", Style::new().bold());
	let l2 = Line::styled("cyan text", Style::new().cyan());
	let l3 = Line::styled("dim text", Style::new().dim());
	let paragraph = Paragraph::new(vec![l0, l1, l2, l3]);

	let mut expected = Buffer::with_lines(["unformatted", "bold text", "cyan text", "dim text"]);
	expected.set_style(Rect::new(0, 1, 9, 1), Style::new().bold());
	expected.set_style(Rect::new(0, 2, 9, 1), Style::new().cyan());
	expected.set_style(Rect::new(0, 3, 8, 1), Style::new().dim());

	test_case(&paragraph, &expected);
}

#[test]
fn test_render_line_spans_styled() {
	let l0 = Line::default().spans([
		Span::styled("bold", Style::new().bold()),
		Span::raw(" and "),
		Span::styled("cyan", Style::new().cyan()),
	]);
	let l1 = Line::default().spans([Span::raw("unformatted")]);
	let paragraph = Paragraph::new(vec![l0, l1]);

	let mut expected = Buffer::with_lines(["bold and cyan", "unformatted"]);
	expected.set_style(Rect::new(0, 0, 4, 1), Style::new().bold());
	expected.set_style(Rect::new(9, 0, 4, 1), Style::new().cyan());

	test_case(&paragraph, &expected);
}

#[test]
fn test_render_paragraph_with_block_with_bottom_title_and_border() {
	let block = Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::BOTTOM)
		.title_position(TitlePosition::Bottom)
		.title("Title");
	let paragraph = Paragraph::new("Hello, world!").block(block);
	test_case(
		&paragraph,
		&Buffer::with_lines(["Hello, world!  ", "Title──────────"]),
	);
}

#[test]
fn test_render_paragraph_with_word_wrap() {
	let text = "This is a long line of text that should wrap      and contains a superultramegagigalong word.";
	let wrapped_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
	let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines([
			"This is a long line",
			"of text that should",
			"wrap      and      ",
			"contains a         ",
			"superultramegagigal",
			"ong word.          ",
		]),
	);
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines([
			"This is a   ",
			"long line of",
			"text that   ",
			"should wrap ",
			"    and     ",
			"contains a  ",
			"superultrame",
			"gagigalong  ",
			"word.       ",
		]),
	);

	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines([
			"This is a long line",
			"of text that should",
			"wrap      and      ",
			"contains a         ",
			"superultramegagigal",
			"ong word.          ",
		]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines([
			"This is a   ",
			"long line of",
			"text that   ",
			"should wrap ",
			"and contains",
			"a           ",
			"superultrame",
			"gagigalong  ",
			"word.       ",
		]),
	);
}

#[test]
fn test_render_wrapped_paragraph_with_whitespace_only_line() {
	let text: Text = ["A", "  ", "B", "  a", "C"]
		.into_iter()
		.map(Line::from)
		.collect();
	let paragraph = Paragraph::new(text.clone()).wrap(Wrap { trim: false });
	let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

	test_case(
		&paragraph,
		&Buffer::with_lines(["A", "  ", "B", "  a", "C"]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines(["A", "", "B", "a", "C"]),
	);
}

#[test]
fn test_render_paragraph_with_line_truncation() {
	let text = "This is a long line of text that should be truncated.";
	let truncated_paragraph = Paragraph::new(text);

	test_case(
		&truncated_paragraph,
		&Buffer::with_lines(["This is a long line of"]),
	);
	test_case(
		&truncated_paragraph,
		&Buffer::with_lines(["This is a long line of te"]),
	);
	test_case(
		&truncated_paragraph,
		&Buffer::with_lines(["This is a long line of "]),
	);
	test_case(
		&truncated_paragraph.clone().scroll((0, 2)),
		&Buffer::with_lines(["is is a long line of te"]),
	);
}

#[test]
fn test_render_paragraph_with_left_alignment() {
	let text = "Hello, world!";
	let truncated_paragraph = Paragraph::new(text).alignment(Alignment::Left);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines(["Hello, world!  "]));
		test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines(["Hello,    ", "world!    "]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines(["Hello,    ", "world!    "]),
	);
}

#[test]
fn test_render_paragraph_with_center_alignment() {
	let text = "Hello, world!";
	let truncated_paragraph = Paragraph::new(text).alignment(Alignment::Center);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines([" Hello, world! "]));
		test_case(paragraph, &Buffer::with_lines(["  Hello, world! "]));
		test_case(paragraph, &Buffer::with_lines(["  Hello, world!  "]));
		test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines(["  Hello,  ", "  world!  "]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines(["  Hello,  ", "  world!  "]),
	);
}

#[test]
fn test_render_paragraph_with_right_alignment() {
	let text = "Hello, world!";
	let truncated_paragraph = Paragraph::new(text).alignment(Alignment::Right);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines(["  Hello, world!"]));
		test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines(["    Hello,", "    world!"]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines(["    Hello,", "    world!"]),
	);
}

#[test]
fn test_render_paragraph_with_scroll_offset() {
	let text = "This is a\ncool\nmultiline\nparagraph.";
	let truncated_paragraph = Paragraph::new(text).scroll((2, 0));
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(
			paragraph,
			&Buffer::with_lines(["multiline   ", "paragraph.  ", "            "]),
		);
		test_case(paragraph, &Buffer::with_lines(["multiline   "]));
	}

	test_case(
		&truncated_paragraph.clone().scroll((2, 4)),
		&Buffer::with_lines(["iline   ", "graph.  "]),
	);
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines(["cool   ", "multili", "ne     "]),
	);
}

#[test]
fn test_render_paragraph_with_zero_width_area() {
	let text = "Hello, world!";
	let area = Rect::new(0, 0, 0, 3);

	for paragraph in [
		Paragraph::new(text),
		Paragraph::new(text).wrap(Wrap { trim: false }),
		Paragraph::new(text).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::empty(area));
		test_case(&paragraph.clone().scroll((2, 4)), &Buffer::empty(area));
	}
}

#[test]
fn test_render_paragraph_with_zero_height_area() {
	let text = "Hello, world!";
	let area = Rect::new(0, 0, 10, 0);

	for paragraph in [
		Paragraph::new(text),
		Paragraph::new(text).wrap(Wrap { trim: false }),
		Paragraph::new(text).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::empty(area));
		test_case(&paragraph.clone().scroll((2, 4)), &Buffer::empty(area));
	}
}

#[test]
fn test_render_paragraph_with_styled_text() {
	let text = Line::from(vec![
		Span::styled("Hello, ", Style::default().fg(Color::Red)),
		Span::styled("world!", Style::default().fg(Color::Blue)),
	]);

	let mut expected_buffer = Buffer::with_lines(["Hello, world!"]);
	expected_buffer.set_style(
		Rect::new(0, 0, 7, 1),
		Style::default().fg(Color::Red).bg(Color::Green),
	);
	expected_buffer.set_style(
		Rect::new(7, 0, 6, 1),
		Style::default().fg(Color::Blue).bg(Color::Green),
	);

	for paragraph in [
		Paragraph::new(text.clone()),
		Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
		Paragraph::new(text.clone()).wrap(Wrap { trim: true }),
	] {
		test_case(
			&paragraph.style(Style::default().bg(Color::Green)),
			&expected_buffer,
		);
	}
}

#[test]
fn test_render_paragraph_with_special_characters() {
	let text = "Hello, <world>!";
	for paragraph in [
		Paragraph::new(text),
		Paragraph::new(text).wrap(Wrap { trim: false }),
		Paragraph::new(text).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!"]));
		test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!     "]));
		test_case(
			&paragraph,
			&Buffer::with_lines(["Hello, <world>!     ", "                    "]),
		);
		test_case(
			&paragraph,
			&Buffer::with_lines(["Hello, <world>!", "               "]),
		);
	}
}

#[test]
fn test_render_paragraph_with_unicode_characters() {
	let text = "こんにちは, 世界! 😃";
	let truncated_paragraph = Paragraph::new(text);
	let wrapped_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
	let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines(["こんにちは, 世界! 😃"]));
		test_case(
			paragraph,
			&Buffer::with_lines(["こんにちは, 世界! 😃     "]),
		);
	}

	test_case(
		&truncated_paragraph,
		&Buffer::with_lines(["こんにちは, 世 "]),
	);
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines(["こんにちは,    ", "世界! 😃      "]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines(["こんにちは,    ", "世界! 😃      "]),
	);
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
fn widgets_paragraph_count_rendered_lines() {
	let paragraph = Paragraph::new("Hello World");
	assert_eq!(paragraph.line_count(20), 1);
	assert_eq!(paragraph.line_count(10), 1);
	let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_count(20), 1);
	assert_eq!(paragraph.line_count(10), 2);
	let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_count(20), 1);
	assert_eq!(paragraph.line_count(10), 2);

	let text = "Hello World ".repeat(100);
	let paragraph = Paragraph::new(text.trim());
	assert_eq!(paragraph.line_count(11), 1);
	assert_eq!(paragraph.line_count(6), 1);
	let paragraph = paragraph.wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_count(11), 100);
	assert_eq!(paragraph.line_count(6), 200);
	let paragraph = paragraph.wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_count(11), 100);
	assert_eq!(paragraph.line_count(6), 200);
}

#[test]
fn widgets_paragraph_rendered_line_count_accounts_block() {
	let block = Block::new();
	let paragraph = Paragraph::new("Hello World").block(block);
	assert_eq!(paragraph.line_count(20), 1);
	assert_eq!(paragraph.line_count(10), 1);

	let block = Block::new().borders(Borders::TOP);
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(20), 2);
	assert_eq!(paragraph.line_count(10), 2);

	let block = Block::new().borders(Borders::BOTTOM);
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(20), 2);
	assert_eq!(paragraph.line_count(10), 2);

	let block = Block::new().borders(Borders::TOP | Borders::BOTTOM);
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(20), 3);
	assert_eq!(paragraph.line_count(10), 3);

	let block = Block::bordered();
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(20), 3);
	assert_eq!(paragraph.line_count(10), 3);

	let block = Block::bordered();
	let paragraph = paragraph.block(block).wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_count(20), 3);
	assert_eq!(paragraph.line_count(10), 4);

	let block = Block::bordered();
	let paragraph = paragraph.block(block).wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_count(20), 3);
	assert_eq!(paragraph.line_count(10), 4);

	let text = "Hello World ".repeat(100);
	let block = Block::new();
	let paragraph = Paragraph::new(text.trim()).block(block);
	assert_eq!(paragraph.line_count(11), 1);

	let block = Block::bordered();
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(11), 3);
	assert_eq!(paragraph.line_count(6), 3);

	let block = Block::new().borders(Borders::TOP);
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(11), 2);
	assert_eq!(paragraph.line_count(6), 2);

	let block = Block::new().borders(Borders::BOTTOM);
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(11), 2);
	assert_eq!(paragraph.line_count(6), 2);

	let block = Block::new().borders(Borders::LEFT | Borders::RIGHT);
	let paragraph = paragraph.block(block);
	assert_eq!(paragraph.line_count(11), 1);
	assert_eq!(paragraph.line_count(6), 1);
}

#[test]
fn widgets_paragraph_line_width() {
	let paragraph = Paragraph::new("Hello World");
	assert_eq!(paragraph.line_width(), 11);
	let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_width(), 11);
	let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_width(), 11);

	let text = "Hello World ".repeat(100);
	let paragraph = Paragraph::new(text);
	assert_eq!(paragraph.line_width(), 1200);
	let paragraph = paragraph.wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_width(), 1200);
	let paragraph = paragraph.wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_width(), 1200);
}

#[test]
fn widgets_paragraph_line_width_accounts_for_block() {
	let block = Block::bordered();
	let paragraph = Paragraph::new("Hello World").block(block);
	assert_eq!(paragraph.line_width(), 13);

	let block = Block::new().borders(Borders::LEFT);
	let paragraph = Paragraph::new("Hello World").block(block);
	assert_eq!(paragraph.line_width(), 12);

	let block = Block::new().borders(Borders::LEFT);
	let paragraph = Paragraph::new("Hello World")
		.block(block)
		.wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_width(), 12);

	let block = Block::new().borders(Borders::LEFT);
	let paragraph = Paragraph::new("Hello World")
		.block(block)
		.wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_width(), 12);
}

#[test]
fn left_aligned() {
	let p = Paragraph::new("Hello, world!").left_aligned();
	assert_eq!(p.alignment, Alignment::Left);
}

#[test]
fn centered() {
	let p = Paragraph::new("Hello, world!").centered();
	assert_eq!(p.alignment, Alignment::Center);
}

#[test]
fn right_aligned() {
	let p = Paragraph::new("Hello, world!").right_aligned();
	assert_eq!(p.alignment, Alignment::Right);
}

/// Regression test for
///
/// This test ensures that paragraphs with a block and styled text are rendered correctly.
/// It has been simplified from the original issue but tests the same functionality.
#[test]
fn paragraph_block_text_style() {
	let text = Text::styled("Styled text", Color::Green);
	let paragraph = Paragraph::new(text).block(Block::bordered().border_type(BorderType::Plain));

	let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
	paragraph.render(Rect::new(0, 0, 20, 3), &mut buf);

	let mut expected = Buffer::with_lines([
		"┌──────────────────┐",
		"│Styled text       │",
		"└──────────────────┘",
	]);
	expected.set_style(Rect::new(1, 1, 11, 1), Style::default().fg(Color::Green));
	assert_eq!(buf, expected);
}

#[rstest]
#[case::bottom(Rect::new(0, 5, 15, 1))]
#[case::right(Rect::new(20, 0, 15, 1))]
#[case::bottom_right(Rect::new(20, 5, 15, 1))]
fn test_render_paragraph_out_of_bounds(#[case] area: Rect) {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	Paragraph::new("Beyond the pale").render(area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(vec!["          "; 3]));
}

#[test]
fn partial_out_of_bounds() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 15, 3));
	Paragraph::new("Hello World").render(Rect::new(10, 0, 10, 3), &mut buffer);
	assert_eq!(
		buffer,
		Buffer::with_lines(vec![
			"          Hello",
			"               ",
			"               ",
		])
	);
}

#[test]
fn render_in_minimal_buffer() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
	let paragraph = Paragraph::new("Lorem ipsum");
	// This should not panic, even if the buffer is too small to render the paragraph.
	paragraph.render(buffer.area, &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["L"]));
}

#[test]
fn render_in_zero_size_buffer() {
	let mut buffer = Buffer::empty(Rect::ZERO);
	let paragraph = Paragraph::new("Lorem ipsum");
	// This should not panic, even if the buffer has zero size.
	paragraph.render(buffer.area, &mut buffer);
}
