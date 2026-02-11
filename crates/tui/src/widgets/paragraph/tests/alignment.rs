//! Tests for Paragraph alignment functionality.

use super::*;

#[test]
fn test_render_paragraph_with_left_alignment() {
	let text = "Hello, world!";
	let truncated_paragraph = Paragraph::new(text).alignment(HorizontalAlignment::Left);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines(["Hello, world!  "]));
		test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
	test_case(&wrapped_paragraph, &Buffer::with_lines(["Hello,    ", "world!    "]));
	test_case(&trimmed_paragraph, &Buffer::with_lines(["Hello,    ", "world!    "]));
}

#[test]
fn test_render_paragraph_with_center_alignment() {
	let text = "Hello, world!";
	let truncated_paragraph = Paragraph::new(text).alignment(HorizontalAlignment::Center);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines([" Hello, world! "]));
		test_case(paragraph, &Buffer::with_lines(["  Hello, world! "]));
		test_case(paragraph, &Buffer::with_lines(["  Hello, world!  "]));
		test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
	test_case(&wrapped_paragraph, &Buffer::with_lines(["  Hello,  ", "  world!  "]));
	test_case(&trimmed_paragraph, &Buffer::with_lines(["  Hello,  ", "  world!  "]));
}

#[test]
fn test_render_paragraph_with_right_alignment() {
	let text = "Hello, world!";
	let truncated_paragraph = Paragraph::new(text).alignment(HorizontalAlignment::Right);
	let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
	let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

	for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
		test_case(paragraph, &Buffer::with_lines(["  Hello, world!"]));
		test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
	test_case(&wrapped_paragraph, &Buffer::with_lines(["    Hello,", "    world!"]));
	test_case(&trimmed_paragraph, &Buffer::with_lines(["    Hello,", "    world!"]));
}
