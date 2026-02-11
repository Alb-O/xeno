//! Edge case tests for Paragraph widget (zero size, unicode, out of bounds, etc.).

use super::*;

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
fn test_render_paragraph_with_special_characters() {
	let text = "Hello, <world>!";
	for paragraph in [
		Paragraph::new(text),
		Paragraph::new(text).wrap(Wrap { trim: false }),
		Paragraph::new(text).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!"]));
		test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!     "]));
		test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!     ", "                    "]));
		test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!", "               "]));
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
		test_case(paragraph, &Buffer::with_lines(["こんにちは, 世界! 😃     "]));
	}

	test_case(&truncated_paragraph, &Buffer::with_lines(["こんにちは, 世 "]));
	test_case(&wrapped_paragraph, &Buffer::with_lines(["こんにちは,    ", "世界! 😃      "]));
	test_case(&trimmed_paragraph, &Buffer::with_lines(["こんにちは,    ", "世界! 😃      "]));
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
	assert_eq!(buffer, Buffer::with_lines(vec!["          Hello", "               ", "               ",]));
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
