//! Tests for Text iterator implementations.

use rstest::{fixture, rstest};

use super::*;

/// a fixture used in the tests below to avoid repeating the same setup
#[fixture]
fn hello_world() -> Text<'static> {
	Text::from(vec![
		Line::styled("Hello ", Color::Blue),
		Line::styled("world!", Color::Green),
	])
}

#[rstest]
fn iter(hello_world: Text<'_>) {
	let mut iter = hello_world.iter();
	assert_eq!(iter.next(), Some(&Line::styled("Hello ", Color::Blue)));
	assert_eq!(iter.next(), Some(&Line::styled("world!", Color::Green)));
	assert_eq!(iter.next(), None);
}

#[rstest]
fn iter_mut(mut hello_world: Text<'_>) {
	let mut iter = hello_world.iter_mut();
	assert_eq!(iter.next(), Some(&mut Line::styled("Hello ", Color::Blue)));
	assert_eq!(iter.next(), Some(&mut Line::styled("world!", Color::Green)));
	assert_eq!(iter.next(), None);
}

#[rstest]
fn into_iter(hello_world: Text<'_>) {
	let mut iter = hello_world.into_iter();
	assert_eq!(iter.next(), Some(Line::styled("Hello ", Color::Blue)));
	assert_eq!(iter.next(), Some(Line::styled("world!", Color::Green)));
	assert_eq!(iter.next(), None);
}

#[rstest]
fn into_iter_ref(hello_world: Text<'_>) {
	let mut iter = (&hello_world).into_iter();
	assert_eq!(iter.next(), Some(&Line::styled("Hello ", Color::Blue)));
	assert_eq!(iter.next(), Some(&Line::styled("world!", Color::Green)));
	assert_eq!(iter.next(), None);
}

#[test]
fn into_iter_mut_ref() {
	let mut hello_world = Text::from(vec![
		Line::styled("Hello ", Color::Blue),
		Line::styled("world!", Color::Green),
	]);
	let mut iter = (&mut hello_world).into_iter();
	assert_eq!(iter.next(), Some(&mut Line::styled("Hello ", Color::Blue)));
	assert_eq!(iter.next(), Some(&mut Line::styled("world!", Color::Green)));
	assert_eq!(iter.next(), None);
}

#[rstest]
fn for_loop_ref(hello_world: Text<'_>) {
	let mut result = String::new();
	for line in &hello_world {
		result.push_str(line.to_string().as_ref());
	}
	assert_eq!(result, "Hello world!");
}

#[rstest]
fn for_loop_mut_ref() {
	let mut hello_world = Text::from(vec![
		Line::styled("Hello ", Color::Blue),
		Line::styled("world!", Color::Green),
	]);
	let mut result = String::new();
	for line in &mut hello_world {
		result.push_str(line.to_string().as_ref());
	}
	assert_eq!(result, "Hello world!");
}

#[rstest]
fn for_loop_into(hello_world: Text<'_>) {
	let mut result = String::new();
	for line in hello_world {
		result.push_str(line.to_string().as_ref());
	}
	assert_eq!(result, "Hello world!");
}
