use ropey::Rope;

use super::*;

#[test]
fn test_find_char_forward() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = find_char_forward(slice, range, 'o', 1, true, false);
	assert_eq!(moved.head, 4);

	let moved = find_char_forward(slice, range, 'o', 1, false, false);
	assert_eq!(moved.head, 3);
}

#[test]
fn test_find_char_forward_count() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(0);

	let moved = find_char_forward(slice, range, 'o', 2, true, false);
	assert_eq!(moved.head, 7);
}

#[test]
fn test_find_char_backward() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);
	let range = Range::point(10);

	let moved = find_char_backward(slice, range, 'o', 1, true, false);
	assert_eq!(moved.head, 7);
}

#[test]
fn test_is_word_char() {
	assert!(is_word_char('a'));
	assert!(is_word_char('Z'));
	assert!(is_word_char('0'));
	assert!(is_word_char('_'));
	assert!(!is_word_char(' '));
	assert!(!is_word_char('.'));
}
