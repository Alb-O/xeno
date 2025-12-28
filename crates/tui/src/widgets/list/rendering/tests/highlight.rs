//! Highlight symbol and style tests

use rstest::rstest;

use super::*;

#[test]
fn highlight_symbol_and_style() {
	let list = List::new(["Item 0", "Item 1", "Item 2"])
		.highlight_symbol(">>")
		.highlight_style(Style::default().fg(Color::Yellow));
	let mut state = ListState::default();
	state.select(Some(1));
	let buffer = stateful_widget(list, &mut state, 10, 5);
	let expected = Buffer::with_lines([
		"  Item 0  ".into(),
		">>Item 1  ".yellow(),
		"  Item 2  ".into(),
		"          ".into(),
		"          ".into(),
	]);
	assert_eq!(buffer, expected);
}

#[test]
fn highlight_symbol_style_and_style() {
	let list = List::new(["Item 0", "Item 1", "Item 2"])
		.highlight_symbol(Line::from(">>").red().bold())
		.highlight_style(Style::default().fg(Color::Yellow));
	let mut state = ListState::default();
	state.select(Some(1));
	let buffer = stateful_widget(list, &mut state, 10, 5);
	let mut expected = Buffer::with_lines([
		"  Item 0  ".into(),
		">>Item 1  ".yellow(),
		"  Item 2  ".into(),
		"          ".into(),
		"          ".into(),
	]);
	expected.set_style(Rect::new(0, 1, 2, 1), Style::new().red().bold());
	assert_eq!(buffer, expected);
}

#[test]
fn highlight_spacing_default_when_selected() {
	// when not selected
	{
		let list = List::new(["Item 0", "Item 1", "Item 2"]).highlight_symbol(">>");
		let mut state = ListState::default();
		let buffer = stateful_widget(list, &mut state, 10, 5);
		let expected = Buffer::with_lines([
			"Item 0    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
			"          ",
		]);
		assert_eq!(buffer, expected);
	}

	// when selected
	{
		let list = List::new(["Item 0", "Item 1", "Item 2"]).highlight_symbol(">>");
		let mut state = ListState::default();
		state.select(Some(1));
		let buffer = stateful_widget(list, &mut state, 10, 5);
		let expected = Buffer::with_lines([
			"  Item 0  ",
			">>Item 1  ",
			"  Item 2  ",
			"          ",
			"          ",
		]);
		assert_eq!(buffer, expected);
	}
}

#[test]
fn highlight_spacing_default_always() {
	// when not selected
	{
		let list = List::new(["Item 0", "Item 1", "Item 2"])
			.highlight_symbol(">>")
			.highlight_spacing(HighlightSpacing::Always);
		let mut state = ListState::default();
		let buffer = stateful_widget(list, &mut state, 10, 5);
		let expected = Buffer::with_lines([
			"  Item 0  ",
			"  Item 1  ",
			"  Item 2  ",
			"          ",
			"          ",
		]);
		assert_eq!(buffer, expected);
	}

	// when selected
	{
		let list = List::new(["Item 0", "Item 1", "Item 2"])
			.highlight_symbol(">>")
			.highlight_spacing(HighlightSpacing::Always);
		let mut state = ListState::default();
		state.select(Some(1));
		let buffer = stateful_widget(list, &mut state, 10, 5);
		let expected = Buffer::with_lines([
			"  Item 0  ",
			">>Item 1  ",
			"  Item 2  ",
			"          ",
			"          ",
		]);
		assert_eq!(buffer, expected);
	}
}

#[test]
fn highlight_spacing_default_never() {
	// when not selected
	{
		let list = List::new(["Item 0", "Item 1", "Item 2"])
			.highlight_symbol(">>")
			.highlight_spacing(HighlightSpacing::Never);
		let mut state = ListState::default();
		let buffer = stateful_widget(list, &mut state, 10, 5);
		let expected = Buffer::with_lines([
			"Item 0    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
			"          ",
		]);
		assert_eq!(buffer, expected);
	}

	// when selected
	{
		let list = List::new(["Item 0", "Item 1", "Item 2"])
			.highlight_symbol(">>")
			.highlight_spacing(HighlightSpacing::Never);
		let mut state = ListState::default();
		state.select(Some(1));
		let buffer = stateful_widget(list, &mut state, 10, 5);
		let expected = Buffer::with_lines([
			"Item 0    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
			"          ",
		]);
		assert_eq!(buffer, expected);
	}
}

#[test]
fn repeat_highlight_symbol() {
	let list = List::new(["Item 0\nLine 2", "Item 1", "Item 2"])
		.highlight_symbol(Line::from(">>").red().bold())
		.highlight_style(Style::default().fg(Color::Yellow))
		.repeat_highlight_symbol(true);
	let mut state = ListState::default();
	state.select(Some(0));
	let buffer = stateful_widget(list, &mut state, 10, 5);
	let mut expected = Buffer::with_lines([
		">>Item 0  ".yellow(),
		">>Line 2  ".yellow(),
		"  Item 1  ".into(),
		"  Item 2  ".into(),
		"          ".into(),
	]);
	expected.set_style(Rect::new(0, 0, 2, 2), Style::new().red().bold());
	assert_eq!(buffer, expected);
}

/// Regression test for a bug where highlight symbol being greater than width caused a panic due
/// to subtraction with underflow.
///
/// See [#949]() for details
#[rstest]
#[case::under(">>>>", "Item1", ">>>>Item1 ")] // enough space to render the highlight symbol
#[case::exact(">>>>>", "Item1", ">>>>>Item1")] // exact space to render the highlight symbol
#[case::overflow(">>>>>>", "Item1", ">>>>>>Item")] // not enough space
fn highlight_symbol_overflow(
	#[case] highlight_symbol: &str,
	#[case] item: &str,
	#[case] expected: &str,
	mut single_line_buf: Buffer,
) {
	let list = List::new([item]).highlight_symbol(highlight_symbol);
	let mut state = ListState::default();
	state.select(Some(0));
	StatefulWidget::render(list, single_line_buf.area, &mut single_line_buf, &mut state);
	assert_eq!(single_line_buf, Buffer::with_lines([expected]));
}
