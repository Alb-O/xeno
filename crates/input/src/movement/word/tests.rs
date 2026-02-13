use ropey::Rope;

use super::*;

#[test]
fn test_move_to_next_word_start() {
	let text = Rope::from("hello world test");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_to_next_word_start(slice, range, 1, WordType::Word, false);
	assert_eq!(moved.head, 6);

	let moved2 = move_to_next_word_start(slice, moved, 1, WordType::Word, false);
	assert_eq!(moved2.head, 12);
}

#[test]
fn test_move_to_next_word_start_count() {
	let text = Rope::from("one two three four");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_to_next_word_start(slice, range, 2, WordType::Word, false);
	assert_eq!(moved.head, 8);
}

#[test]
fn test_move_to_prev_word_start() {
	let text = Rope::from("hello world test");
	let slice = text.slice(..);
	let range = Range::point(12);

	let moved = move_to_prev_word_start(slice, range, 1, WordType::Word, false);
	assert_eq!(moved.head, 6);
}

#[test]
fn test_move_to_next_word_end() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_to_next_word_end(slice, range, 1, WordType::Word, false);
	assert_eq!(moved.head, 4);
}

#[test]
fn test_word_movement_extend() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = move_to_next_word_start(slice, range, 1, WordType::Word, true);
	assert_eq!(moved.anchor, 0);
	assert_eq!(moved.head, 6);
}
