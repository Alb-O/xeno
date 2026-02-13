use ropey::Rope;

use super::*;

#[test]
fn test_grapheme_boundaries() {
	let text = Rope::from("hello");
	let slice = text.slice(..);

	assert!(is_grapheme_boundary(slice, 0));
	assert!(is_grapheme_boundary(slice, 5));
	assert_eq!(next_grapheme_boundary(slice, 0), 1);
	assert_eq!(prev_grapheme_boundary(slice, 5), 4);
}

#[test]
fn test_emoji_graphemes() {
	let text = Rope::from("a😀b");
	let slice = text.slice(..);

	assert!(is_grapheme_boundary(slice, 0));
	assert!(is_grapheme_boundary(slice, 1));
	assert_eq!(next_grapheme_boundary(slice, 1), 2);
	assert_eq!(next_grapheme_boundary(slice, 2), 3);
}
