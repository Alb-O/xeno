//! Tests for Color From trait implementations.

use super::*;

#[test]
fn from_array_and_tuple_conversions() {
	let from_array3 = Color::from([123, 45, 67]);
	assert_eq!(from_array3, Color::Rgb(123, 45, 67));

	let from_tuple3 = Color::from((89, 76, 54));
	assert_eq!(from_tuple3, Color::Rgb(89, 76, 54));

	let from_array4 = Color::from([10, 20, 30, 255]);
	assert_eq!(from_array4, Color::Rgb(10, 20, 30));

	let from_tuple4 = Color::from((200, 150, 100, 0));
	assert_eq!(from_tuple4, Color::Rgb(200, 150, 100));
}
