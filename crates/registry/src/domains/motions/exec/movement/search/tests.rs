use ropey::Rope;

use super::*;

#[test]
fn test_find_next() {
	let text = Rope::from("hello world hello");
	let slice = text.slice(..);

	let m = find_next(slice, "hello", 0).unwrap().unwrap();
	assert_eq!(m.min(), 0);
	assert_eq!(m.max(), 4);

	let m = find_next(slice, "hello", 1).unwrap().unwrap();
	assert_eq!(m.min(), 12);
	assert_eq!(m.max(), 16);

	let m = find_next(slice, "hello", 13).unwrap().unwrap();
	assert_eq!(m.min(), 0);
}

#[test]
fn test_find_prev() {
	let text = Rope::from("hello world hello");
	let slice = text.slice(..);

	let m = find_prev(slice, "hello", 17).unwrap().unwrap();
	assert_eq!(m.min(), 12);

	let m = find_prev(slice, "hello", 12).unwrap().unwrap();
	assert_eq!(m.min(), 0);

	let m = find_prev(slice, "hello", 0).unwrap().unwrap();
	assert_eq!(m.min(), 12);
}

#[test]
fn test_find_all_matches() {
	let text = Rope::from("foo bar foo baz foo");
	let slice = text.slice(..);

	let matches = find_all_matches(slice, "foo").unwrap();
	assert_eq!(matches.len(), 3);
	assert_eq!(matches[0].min(), 0);
	assert_eq!(matches[1].min(), 8);
	assert_eq!(matches[2].min(), 16);
}
