use alloc::borrow::ToOwned;
use alloc::vec;
use alloc::vec::Vec;

use rstest::{fixture, rstest};

use super::*;
use crate::buffer::Buffer;
use crate::layout::{Alignment, Rect};
use crate::style::{Color, Modifier, Style, Stylize};
use crate::text::Line;
use crate::widgets::block::Block;
use crate::widgets::borders::BorderType;
use crate::widgets::list::{List, ListItem, ListState};
use crate::widgets::table::HighlightSpacing;
use crate::widgets::{StatefulWidget, Widget};

mod alignment;
mod highlight;
mod rendering;
mod scrolling;

#[fixture]
pub(super) fn single_line_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 1))
}

/// helper method to render a widget to an empty buffer with the default state
pub(super) fn widget(widget: List<'_>, width: u16, height: u16) -> Buffer {
	let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
	Widget::render(widget, buffer.area, &mut buffer);
	buffer
}

/// helper method to render a widget to an empty buffer with a given state
pub(super) fn stateful_widget(
	widget: List<'_>,
	state: &mut ListState,
	width: u16,
	height: u16,
) -> Buffer {
	let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
	StatefulWidget::render(widget, buffer.area, &mut buffer, state);
	buffer
}

#[rstest]
fn empty_list(mut single_line_buf: Buffer) {
	let mut state = ListState::default();

	let items: Vec<ListItem> = Vec::new();
	let list = List::new(items);
	state.select_first();
	StatefulWidget::render(list, single_line_buf.area, &mut single_line_buf, &mut state);
	assert_eq!(state.selected, None);
}

#[rstest]
fn single_item(mut single_line_buf: Buffer) {
	let mut state = ListState::default();

	let items = vec![ListItem::new("Item 1")];
	let list = List::new(items);
	state.select_first();
	StatefulWidget::render(
		&list,
		single_line_buf.area,
		&mut single_line_buf,
		&mut state,
	);
	assert_eq!(state.selected, Some(0));

	state.select_last();
	StatefulWidget::render(
		&list,
		single_line_buf.area,
		&mut single_line_buf,
		&mut state,
	);
	assert_eq!(state.selected, Some(0));

	state.select_previous();
	StatefulWidget::render(
		&list,
		single_line_buf.area,
		&mut single_line_buf,
		&mut state,
	);
	assert_eq!(state.selected, Some(0));

	state.select_next();
	StatefulWidget::render(
		&list,
		single_line_buf.area,
		&mut single_line_buf,
		&mut state,
	);
	assert_eq!(state.selected, Some(0));
}

#[expect(clippy::too_many_lines)]
#[test]
fn combinations() {
	#[track_caller]
	fn test_case_render<'line, Lines>(items: &[ListItem], expected: Lines)
	where
		Lines: IntoIterator,
		Lines::Item: Into<Line<'line>>,
	{
		let list = List::new(items.to_owned()).highlight_symbol(">>");
		let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
		Widget::render(list, buffer.area, &mut buffer);
		assert_eq!(buffer, Buffer::with_lines(expected));
	}

	#[track_caller]
	fn test_case_render_stateful<'line, Lines>(
		items: &[ListItem],
		selected: Option<usize>,
		expected: Lines,
	) where
		Lines: IntoIterator,
		Lines::Item: Into<Line<'line>>,
	{
		let list = List::new(items.to_owned()).highlight_symbol(">>");
		let mut state = ListState::default().with_selected(selected);
		let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
		StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);
		assert_eq!(buffer, Buffer::with_lines(expected));
	}

	let empty_items = Vec::new();
	let single_item = vec!["Item 0".into()];
	let multiple_items = vec!["Item 0".into(), "Item 1".into(), "Item 2".into()];
	let multi_line_items = vec!["Item 0\nLine 2".into(), "Item 1".into(), "Item 2".into()];

	// empty list
	test_case_render(
		&empty_items,
		[
			"          ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&empty_items,
		None,
		[
			"          ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&empty_items,
		Some(0),
		[
			"          ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);

	// single item
	test_case_render(
		&single_item,
		[
			"Item 0    ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&single_item,
		None,
		[
			"Item 0    ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&single_item,
		Some(0),
		[
			">>Item 0  ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&single_item,
		Some(1),
		[
			">>Item 0  ",
			"          ",
			"          ",
			"          ",
			"          ",
		],
	);

	// multiple items
	test_case_render(
		&multiple_items,
		[
			"Item 0    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multiple_items,
		None,
		[
			"Item 0    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multiple_items,
		Some(0),
		[
			">>Item 0  ",
			"  Item 1  ",
			"  Item 2  ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multiple_items,
		Some(1),
		[
			"  Item 0  ",
			">>Item 1  ",
			"  Item 2  ",
			"          ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multiple_items,
		Some(3),
		[
			"  Item 0  ",
			"  Item 1  ",
			">>Item 2  ",
			"          ",
			"          ",
		],
	);

	// multi line items
	test_case_render(
		&multi_line_items,
		[
			"Item 0    ",
			"Line 2    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multi_line_items,
		None,
		[
			"Item 0    ",
			"Line 2    ",
			"Item 1    ",
			"Item 2    ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multi_line_items,
		Some(0),
		[
			">>Item 0  ",
			"  Line 2  ",
			"  Item 1  ",
			"  Item 2  ",
			"          ",
		],
	);
	test_case_render_stateful(
		&multi_line_items,
		Some(1),
		[
			"  Item 0  ",
			"  Line 2  ",
			">>Item 1  ",
			"  Item 2  ",
			"          ",
		],
	);
}

#[test]
fn can_be_stylized() {
	assert_eq!(
		List::new::<Vec<&str>>(vec![])
			.black()
			.on_white()
			.bold()
			.not_dim()
			.style,
		Style::default()
			.fg(Color::Black)
			.bg(Color::White)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::DIM)
	);
}
