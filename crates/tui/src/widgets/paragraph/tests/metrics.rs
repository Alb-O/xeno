//! Tests for Paragraph metrics (line_count, line_width).

use super::*;

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
	let paragraph = Paragraph::new("Hello World").block(block).wrap(Wrap { trim: true });
	assert_eq!(paragraph.line_width(), 12);

	let block = Block::new().borders(Borders::LEFT);
	let paragraph = Paragraph::new("Hello World").block(block).wrap(Wrap { trim: false });
	assert_eq!(paragraph.line_width(), 12);
}
