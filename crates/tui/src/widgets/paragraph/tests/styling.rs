//! Tests for Paragraph styling functionality.

use super::*;

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
fn test_render_paragraph_with_styled_text() {
	let text = Line::from(vec![
		Span::styled("Hello, ", Style::default().fg(Color::Red)),
		Span::styled("world!", Style::default().fg(Color::Blue)),
	]);

	let mut expected_buffer = Buffer::with_lines(["Hello, world!"]);
	expected_buffer.set_style(Rect::new(0, 0, 7, 1), Style::default().fg(Color::Red).bg(Color::Green));
	expected_buffer.set_style(Rect::new(7, 0, 6, 1), Style::default().fg(Color::Blue).bg(Color::Green));

	for paragraph in [
		Paragraph::new(text.clone()),
		Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
		Paragraph::new(text.clone()).wrap(Wrap { trim: true }),
	] {
		test_case(&paragraph.style(Style::default().bg(Color::Green)), &expected_buffer);
	}
}
