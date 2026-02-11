//! Basic list rendering tests

use rstest::rstest;

use super::*;
use crate::widgets::list::ListDirection;

#[test]
fn does_not_render_in_small_space() {
	let items = vec!["Item 0", "Item 1", "Item 2"];
	let list = List::new(items.clone()).highlight_symbol(">>");
	let mut buffer = Buffer::empty(Rect::new(0, 0, 15, 3));

	// attempt to render into an area of the buffer with 0 width
	Widget::render(list.clone(), Rect::new(0, 0, 0, 3), &mut buffer);
	assert_eq!(&buffer, &Buffer::empty(buffer.area));

	// attempt to render into an area of the buffer with 0 height
	Widget::render(list.clone(), Rect::new(0, 0, 15, 0), &mut buffer);
	assert_eq!(&buffer, &Buffer::empty(buffer.area));

	let list = List::new(items).highlight_symbol(">>").block(Block::bordered().border_type(BorderType::Plain));
	// attempt to render into an area of the buffer with zero height after
	// setting the block borders
	Widget::render(list, Rect::new(0, 0, 15, 2), &mut buffer);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"┌─────────────┐",
		"└─────────────┘",
		"               ",
	]);
	assert_eq!(buffer, expected,);
}

#[test]
fn items() {
	let list = List::default().items(["Item 0", "Item 1", "Item 2"]);
	let buffer = widget(list, 10, 5);
	let expected = Buffer::with_lines(["Item 0    ", "Item 1    ", "Item 2    ", "          ", "          "]);
	assert_eq!(buffer, expected);
}

#[test]
fn empty_strings() {
	let list = List::new(["Item 0", "", "", "Item 1", "Item 2"]).block(Block::bordered().border_type(BorderType::Plain).title("List"));
	let buffer = widget(list, 10, 7);
	let expected = Buffer::with_lines(["┌List────┐", "│Item 0  │", "│        │", "│        │", "│Item 1  │", "│Item 2  │", "└────────┘"]);
	assert_eq!(buffer, expected);
}

#[test]
fn block() {
	let list = List::new(["Item 0", "Item 1", "Item 2"]).block(Block::bordered().border_type(BorderType::Plain).title("List"));
	let buffer = widget(list, 10, 7);
	let expected = Buffer::with_lines(["┌List────┐", "│Item 0  │", "│Item 1  │", "│Item 2  │", "│        │", "│        │", "└────────┘"]);
	assert_eq!(buffer, expected);
}

#[test]
fn style() {
	let list = List::new(["Item 0", "Item 1", "Item 2"]).style(Style::default().fg(Color::Red));
	let buffer = widget(list, 10, 5);
	let expected = Buffer::with_lines([
		"Item 0    ".red(),
		"Item 1    ".red(),
		"Item 2    ".red(),
		"          ".red(),
		"          ".red(),
	]);
	assert_eq!(buffer, expected);
}

#[rstest]
#[case::top_to_bottom(ListDirection::TopToBottom, [
	"Item 0    ",
	"Item 1    ",
	"Item 2    ",
	"          ",
])]
#[case::top_to_bottom(ListDirection::BottomToTop, [
	"          ",
	"Item 2    ",
	"Item 1    ",
	"Item 0    ",
])]
fn list_direction<'line, Lines>(#[case] direction: ListDirection, #[case] expected: Lines)
where
	Lines: IntoIterator,
	Lines::Item: Into<Line<'line>>,
{
	let list = List::new(["Item 0", "Item 1", "Item 2"]).direction(direction);
	let buffer = widget(list, 10, 4);
	assert_eq!(buffer, Buffer::with_lines(expected));
}

#[test]
fn truncate_items() {
	let list = List::new(["Item 0", "Item 1", "Item 2", "Item 3", "Item 4"]);
	let buffer = widget(list, 10, 3);
	#[rustfmt::skip]
	let expected = Buffer::with_lines([
		"Item 0    ",
		"Item 1    ",
		"Item 2    ",
	]);
	assert_eq!(buffer, expected);
}

#[rstest]
#[case(None, [
	"Item 0 with a v",
	"Item 1         ",
	"Item 2         ",
])]
#[case(Some(0), [
	">>Item 0 with a",
	"  Item 1       ",
	"  Item 2       ",
])]
fn long_lines<'line, Lines>(#[case] selected: Option<usize>, #[case] expected: Lines)
where
	Lines: IntoIterator,
	Lines::Item: Into<Line<'line>>,
{
	let items = ["Item 0 with a very long line that will be truncated", "Item 1", "Item 2"];
	let list = List::new(items).highlight_symbol(">>");
	let mut state = ListState::default().with_selected(selected);
	let buffer = stateful_widget(list, &mut state, 15, 3);
	assert_eq!(buffer, Buffer::with_lines(expected));
}
