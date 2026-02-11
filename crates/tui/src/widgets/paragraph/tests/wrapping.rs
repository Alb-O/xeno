//! Tests for Paragraph word wrapping and line truncation.

use super::*;

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
	let text: Text = ["A", "  ", "B", "  a", "C"].into_iter().map(Line::from).collect();
	let paragraph = Paragraph::new(text.clone()).wrap(Wrap { trim: false });
	let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

	test_case(&paragraph, &Buffer::with_lines(["A", "  ", "B", "  a", "C"]));
	test_case(&trimmed_paragraph, &Buffer::with_lines(["A", "", "B", "a", "C"]));
}

#[test]
fn test_render_paragraph_with_line_truncation() {
	let text = "This is a long line of text that should be truncated.";
	let truncated_paragraph = Paragraph::new(text);

	test_case(&truncated_paragraph, &Buffer::with_lines(["This is a long line of"]));
	test_case(&truncated_paragraph, &Buffer::with_lines(["This is a long line of te"]));
	test_case(&truncated_paragraph, &Buffer::with_lines(["This is a long line of "]));
	test_case(&truncated_paragraph.clone().scroll((0, 2)), &Buffer::with_lines(["is is a long line of te"]));
}
