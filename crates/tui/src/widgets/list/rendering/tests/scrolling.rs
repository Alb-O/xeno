//! List scrolling, offset, and padding tests

use rstest::rstest;

use super::*;

#[test]
fn offset_renders_shifted() {
	let list = List::new(["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6"]);
	let mut state = ListState::default().with_offset(3);
	let buffer = stateful_widget(list, &mut state, 6, 3);

	let expected = Buffer::with_lines(["Item 3", "Item 4", "Item 5"]);
	assert_eq!(buffer, expected);
}

#[test]
fn selected_item_ensures_selected_item_is_visible_when_offset_is_before_visible_range() {
	let items = ["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6"];
	let list = List::new(items).highlight_symbol(">>");
	// Set the initial visible range to items 3, 4, and 5
	let mut state = ListState::default().with_selected(Some(1)).with_offset(3);
	let buffer = stateful_widget(list, &mut state, 10, 3);

	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		">>Item 1  ",
		"  Item 2  ",
		"  Item 3  ",
	]);

	assert_eq!(buffer, expected);
	assert_eq!(state.selected, Some(1));
	assert_eq!(state.offset, 1, "did not scroll the selected item into view");
}

#[test]
fn selected_item_ensures_selected_item_is_visible_when_offset_is_after_visible_range() {
	let items = ["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6"];
	let list = List::new(items).highlight_symbol(">>");
	// Set the initial visible range to items 3, 4, and 5
	let mut state = ListState::default().with_selected(Some(6)).with_offset(3);
	let buffer = stateful_widget(list, &mut state, 10, 3);

	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"  Item 4  ",
		"  Item 5  ",
		">>Item 6  ",
	]);

	assert_eq!(buffer, expected);
	assert_eq!(state.selected, Some(6));
	assert_eq!(state.offset, 4, "did not scroll the selected item into view");
}

#[rstest]
#[case::no_padding(
	4,
	2, // Offset
	0, // Padding
	Some(2), // Selected
	[
		">> Item 2 ",
		"   Item 3 ",
		"   Item 4 ",
		"   Item 5 ",
	]
)]
#[case::one_before(
	4,
	2, // Offset
	1, // Padding
	Some(2), // Selected
	[
		"   Item 1 ",
		">> Item 2 ",
		"   Item 3 ",
		"   Item 4 ",
	]
)]
#[case::one_after(
	4,
	1, // Offset
	1, // Padding
	Some(4), // Selected
	[
		"   Item 2 ",
		"   Item 3 ",
		">> Item 4 ",
		"   Item 5 ",
	]
)]
#[case::check_padding_overflow(
	4,
	1, // Offset
	2, // Padding
	Some(4), // Selected
	[
		"   Item 2 ",
		"   Item 3 ",
		">> Item 4 ",
		"   Item 5 ",
	]
)]
#[case::no_padding_offset_behavior(
	5, // Render Area Height
	2, // Offset
	0, // Padding
	Some(3), // Selected
	[
		"   Item 2 ",
		">> Item 3 ",
		"   Item 4 ",
		"   Item 5 ",
		"          ",
	]
)]
#[case::two_before(
	5, // Render Area Height
	2, // Offset
	2, // Padding
	Some(3), // Selected
	[
		"   Item 1 ",
		"   Item 2 ",
		">> Item 3 ",
		"   Item 4 ",
		"   Item 5 ",
	]
)]
#[case::keep_selected_visible(
	4,
	0, // Offset
	4, // Padding
	Some(1), // Selected
	[
		"   Item 0 ",
		">> Item 1 ",
		"   Item 2 ",
		"   Item 3 ",
	]
)]
fn with_padding<'line, Lines>(
	#[case] render_height: u16,
	#[case] offset: usize,
	#[case] padding: usize,
	#[case] selected: Option<usize>,
	#[case] expected: Lines,
) where
	Lines: IntoIterator,
	Lines::Item: Into<Line<'line>>,
{
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, render_height));
	let mut state = ListState::default();

	*state.offset_mut() = offset;
	state.select(selected);

	let list = List::new(["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5"])
		.scroll_padding(padding)
		.highlight_symbol(">> ");
	StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines(expected));
}

/// If there isn't enough room for the selected item and the requested padding the list can jump
/// up and down every frame if something isn't done about it. This code tests to make sure that
/// isn't currently happening
#[test]
fn padding_flicker() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
	let mut state = ListState::default();

	*state.offset_mut() = 2;
	state.select(Some(4));

	let items = ["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6", "Item 7"];
	let list = List::new(items).scroll_padding(3).highlight_symbol(">> ");

	StatefulWidget::render(&list, buffer.area, &mut buffer, &mut state);

	let offset_after_render = state.offset();

	StatefulWidget::render(&list, buffer.area, &mut buffer, &mut state);

	// Offset after rendering twice should remain the same as after once
	assert_eq!(offset_after_render, state.offset());
}

#[test]
fn padding_inconsistent_item_sizes() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
	let mut state = ListState::default().with_offset(0).with_selected(Some(3));

	let items = [
		ListItem::new("Item 0"),
		ListItem::new("Item 1"),
		ListItem::new("Item 2"),
		ListItem::new("Item 3"),
		ListItem::new("Item 4\nTest\nTest"),
		ListItem::new("Item 5"),
	];
	let list = List::new(items).scroll_padding(1).highlight_symbol(">> ");

	StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);

	#[rustfmt::skip]
	let expected = [
		"   Item 1 ",
		"   Item 2 ",
		">> Item 3 ",
	];
	assert_eq!(buffer, Buffer::with_lines(expected));
}

// Tests to make sure when it's pushing back the first visible index value that it doesnt
// include an item that's too large
#[test]
fn padding_offset_pushback_break() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 4));
	let mut state = ListState::default();

	*state.offset_mut() = 1;
	state.select(Some(2));

	let items = [
		ListItem::new("Item 0\nTest\nTest"),
		ListItem::new("Item 1"),
		ListItem::new("Item 2"),
		ListItem::new("Item 3"),
	];
	let list = List::new(items).scroll_padding(2).highlight_symbol(">> ");

	StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);
	#[rustfmt::skip]
	assert_eq!(
		buffer,
		Buffer::with_lines([
			"   Item 1 ",
			">> Item 2 ",
			"   Item 3 ",
			"          "])
	);
}
