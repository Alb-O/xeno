use ropey::Rope;

use super::*;

#[test]
fn test_move_forward() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_horizontally(slice, range, Direction::Forward, 1, false);
	assert_eq!(moved.head, 1);
}

#[test]
fn test_move_backward() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(5);

	let moved = move_horizontally(slice, range, Direction::Backward, 2, false);
	assert_eq!(moved.head, 3);
}

#[test]
fn test_move_forward_extend() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_horizontally(slice, range, Direction::Forward, 5, true);
	assert_eq!(moved.anchor, 0);
	assert_eq!(moved.head, 5);
}

#[test]
fn test_move_down() {
	let text = Rope::from("hello\nworld\n");
	let slice = text.slice(..);
	let range = Range::point(2);

	let moved = move_vertically(slice, range, Direction::Forward, 1, false);
	assert_eq!(moved.head, 8);
}

#[test]
fn test_move_up() {
	let text = Rope::from("hello\nworld\n");
	let slice = text.slice(..);
	let range = Range::point(8);

	let moved = move_vertically(slice, range, Direction::Backward, 1, false);
	assert_eq!(moved.head, 2);
}

#[test]
fn test_move_to_line_start() {
	let text = Rope::from("hello\nworld\n");
	let slice = text.slice(..);
	let range = Range::point(8);

	let moved = move_to_line_start(slice, range, false);
	assert_eq!(moved.head, 6);
}

#[test]
fn test_move_to_line_end() {
	let text = Rope::from("hello\nworld\n");
	let slice = text.slice(..);
	let range = Range::point(6);

	let moved = move_to_line_end(slice, range, false);
	assert_eq!(moved.head, 11);
}

#[test]
fn test_move_to_first_nonwhitespace() {
	let text = Rope::from("  hello\n");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_to_first_nonwhitespace(slice, range, false);
	assert_eq!(moved.head, 2);
}

#[test]
fn test_document_movement() {
	let text = Rope::from("line1\nline2\nline3");
	let slice = text.slice(..);
	let range = Range::point(7);

	let start = move_to_document_start(slice, range, false);
	assert_eq!(start.head, 0);

	let end = move_to_document_end(slice, range, false);
	assert_eq!(end.head, 17);
}
