//! Basic rendering tests for Paragraph widget.

use super::*;

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
		test_case(&paragraph, &Buffer::with_lines(["Hello, world!  ", "               "]));
		test_case(&paragraph, &Buffer::with_lines(["Hello, world!", "             "]));
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
		test_case(&paragraph, &Buffer::with_lines(["This is a ", "multiline ", "paragraph."]));
		test_case(&paragraph, &Buffer::with_lines(["This is a      ", "multiline      ", "paragraph.     "]));
		test_case(
			&paragraph,
			&Buffer::with_lines(["This is a      ", "multiline      ", "paragraph.     ", "               ", "               "]),
		);
	}
}

#[test]
fn test_render_paragraph_with_block() {
	// We use the slightly unconventional "worlds" instead of "world" here to make sure when we
	// can truncate this without triggering the typos linter.
	let text = "Hello, worlds!";
	let truncated_paragraph = Paragraph::new(text).block(Block::bordered().border_type(BorderType::Plain).title("Title"));
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
			&Buffer::with_lines(["┌Title───────────┐", "│Hello, worlds!  │", "└────────────────┘"]),
		);
		test_case(
			paragraph,
			&Buffer::with_lines(["┌Title────────────┐", "│Hello, worlds!   │", "│                 │", "└─────────────────┘"]),
		);
	}

	test_case(
		&truncated_paragraph,
		&Buffer::with_lines(["┌Title───────┐", "│Hello, world│", "│            │", "└────────────┘"]),
	);
	test_case(
		&wrapped_paragraph,
		&Buffer::with_lines(["┌Title──────┐", "│Hello,     │", "│worlds!    │", "└───────────┘"]),
	);
	test_case(
		&trimmed_paragraph,
		&Buffer::with_lines(["┌Title──────┐", "│Hello,     │", "│worlds!    │", "└───────────┘"]),
	);
}

#[test]
fn test_render_paragraph_with_block_with_bottom_title_and_border() {
	let block = Block::new()
		.border_type(BorderType::Plain)
		.borders(Borders::BOTTOM)
		.title_position(TitlePosition::Bottom)
		.title("Title");
	let paragraph = Paragraph::new("Hello, world!").block(block);
	test_case(&paragraph, &Buffer::with_lines(["Hello, world!  ", "Title──────────"]));
}

#[test]
fn test_render_paragraph_with_scroll_offset() {
	let text = "This is a\ncool\nmultiline\nparagraph.";
	let truncated_paragraph = Paragraph::new(text).scroll((2, 0));
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines(["multiline   ", "paragraph.  ", "            "]));
		test_case(paragraph, &Buffer::with_lines(["multiline   "]));
	}

	test_case(&truncated_paragraph.clone().scroll((2, 4)), &Buffer::with_lines(["iline   ", "graph.  "]));
	test_case(&wrapped_paragraph, &Buffer::with_lines(["cool   ", "multili", "ne     "]));
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

	let mut expected = Buffer::with_lines(["┌──────────────────┐", "│Styled text       │", "└──────────────────┘"]);
	expected.set_style(Rect::new(1, 1, 11, 1), Style::default().fg(Color::Green));
	assert_eq!(buf, expected);
}

#[test]
fn wide_grapheme_paints_trailing_blank() {
	let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
	// Pre-fill (1,0) with "X" to ensure the trailing cell is overwritten.
	buf[(1u16, 0u16)].set_symbol("X");

	Paragraph::new("❤️").render(Rect::new(0, 0, 3, 1), &mut buf);

	// Head cell is the grapheme.
	assert_eq!(buf[(0u16, 0u16)].symbol(), "❤️");
	// Trailing cell must no longer be "X" — it should be blank.
	assert_ne!(buf[(1u16, 0u16)].symbol(), "X");
	assert!(buf[(1u16, 0u16)].symbol().trim().is_empty());
}
