//! List alignment tests

use super::*;

#[test]
fn with_alignment() {
	let list = List::new([
		Line::from("Left").alignment(HorizontalAlignment::Left),
		Line::from("Center").alignment(HorizontalAlignment::Center),
		Line::from("Right").alignment(HorizontalAlignment::Right),
	]);
	let buffer = widget(list, 10, 4);
	let expected = Buffer::with_lines(["Left      ", "  Center  ", "     Right", ""]);
	assert_eq!(buffer, expected);
}

#[test]
fn alignment_odd_line_odd_area() {
	let list = List::new([
		Line::from("Odd").alignment(HorizontalAlignment::Left),
		Line::from("Even").alignment(HorizontalAlignment::Center),
		Line::from("Width").alignment(HorizontalAlignment::Right),
	]);
	let buffer = widget(list, 7, 4);
	let expected = Buffer::with_lines(["Odd    ", " Even  ", "  Width", ""]);
	assert_eq!(buffer, expected);
}

#[test]
fn alignment_even_line_even_area() {
	let list = List::new([
		Line::from("Odd").alignment(HorizontalAlignment::Left),
		Line::from("Even").alignment(HorizontalAlignment::Center),
		Line::from("Width").alignment(HorizontalAlignment::Right),
	]);
	let buffer = widget(list, 6, 4);
	let expected = Buffer::with_lines(["Odd   ", " Even ", " Width", ""]);
	assert_eq!(buffer, expected);
}

#[test]
fn alignment_odd_line_even_area() {
	let list = List::new([
		Line::from("Odd").alignment(HorizontalAlignment::Left),
		Line::from("Even").alignment(HorizontalAlignment::Center),
		Line::from("Width").alignment(HorizontalAlignment::Right),
	]);
	let buffer = widget(list, 8, 4);
	let expected = Buffer::with_lines(["Odd     ", "  Even  ", "   Width", ""]);
	assert_eq!(buffer, expected);
}

#[test]
fn alignment_even_line_odd_area() {
	let list = List::new([
		Line::from("Odd").alignment(HorizontalAlignment::Left),
		Line::from("Even").alignment(HorizontalAlignment::Center),
		Line::from("Width").alignment(HorizontalAlignment::Right),
	]);
	let buffer = widget(list, 6, 4);
	let expected = Buffer::with_lines(["Odd   ", " Even ", " Width", ""]);
	assert_eq!(buffer, expected);
}

#[test]
fn alignment_zero_line_width() {
	let list = List::new([Line::from("This line has zero width").alignment(HorizontalAlignment::Center)]);
	let buffer = widget(list, 0, 2);
	assert_eq!(buffer, Buffer::with_lines([""; 2]));
}

#[test]
fn alignment_zero_area_width() {
	let list = List::new([Line::from("Text").alignment(HorizontalAlignment::Left)]);
	let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
	Widget::render(list, Rect::new(0, 0, 4, 0), &mut buffer);
	assert_eq!(buffer, Buffer::with_lines(["    "]));
}

#[test]
fn alignment_line_less_than_width() {
	let list = List::new([Line::from("Small").alignment(HorizontalAlignment::Center)]);
	let buffer = widget(list, 10, 2);
	let expected = Buffer::with_lines(["  Small   ", ""]);
	assert_eq!(buffer, expected);
}

#[test]
fn alignment_line_equal_to_width() {
	let list = List::new([Line::from("Exact").alignment(HorizontalAlignment::Left)]);
	let buffer = widget(list, 5, 2);
	assert_eq!(buffer, Buffer::with_lines(["Exact", ""]));
}

#[test]
fn alignment_line_greater_than_width() {
	let list = List::new([Line::from("Large line").alignment(HorizontalAlignment::Left)]);
	let buffer = widget(list, 5, 2);
	assert_eq!(buffer, Buffer::with_lines(["Large", ""]));
}
