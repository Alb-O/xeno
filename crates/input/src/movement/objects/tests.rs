use ropey::Rope;

use super::*;

#[test]
fn test_select_word_object_inner() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);

	let range = Range::point(1);
	let selected = select_word_object(slice, range, WordType::Word, true);
	assert_eq!(selected.min(), 0);
	assert_eq!(selected.max(), 4);

	let range = Range::point(7);
	let selected = select_word_object(slice, range, WordType::Word, true);
	assert_eq!(selected.min(), 6);
	assert_eq!(selected.max(), 10);
}

#[test]
fn test_select_word_object_around() {
	let text = Rope::from("hello world");
	let slice = text.slice(..);

	let range = Range::point(1);
	let selected = select_word_object(slice, range, WordType::Word, false);
	assert_eq!(selected.min(), 0);
	assert_eq!(selected.max(), 5);
}

#[test]
fn test_select_surround_object_parens() {
	let text = Rope::from("foo(bar)baz");
	let slice = text.slice(..);

	let range = Range::point(5);

	let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
	assert_eq!(selected.min(), 4);
	assert_eq!(selected.max(), 6);

	let selected = select_surround_object(slice, range, '(', ')', false).unwrap();
	assert_eq!(selected.min(), 3);
	assert_eq!(selected.max(), 7);
}

#[test]
fn test_select_surround_object_nested() {
	let text = Rope::from("foo(a(b)c)bar");
	let slice = text.slice(..);

	let range = Range::point(6);
	let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
	assert_eq!(selected.min(), 6);
	assert_eq!(selected.max(), 6);

	let range = Range::point(4);
	let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
	assert_eq!(selected.min(), 4);
	assert_eq!(selected.max(), 8);
}

#[test]
fn test_select_surround_object_quotes() {
	let text = Rope::from(r#"say "hello" now"#);
	let slice = text.slice(..);

	let range = Range::point(6);

	let selected = select_surround_object(slice, range, '"', '"', true).unwrap();
	assert_eq!(selected.min(), 5);
	assert_eq!(selected.max(), 9);

	let selected = select_surround_object(slice, range, '"', '"', false).unwrap();
	assert_eq!(selected.min(), 4);
	assert_eq!(selected.max(), 10);
}
