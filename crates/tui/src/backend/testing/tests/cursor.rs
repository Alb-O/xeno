//! Cursor-related tests for `TestBackend`.

use super::*;

#[test]
fn hide_cursor() {
	let mut backend = TestBackend::new(10, 2);
	backend.hide_cursor().unwrap();
	assert!(!backend.cursor);
}

#[test]
fn show_cursor() {
	let mut backend = TestBackend::new(10, 2);
	backend.show_cursor().unwrap();
	assert!(backend.cursor);
}

#[test]
fn get_cursor_position() {
	let mut backend = TestBackend::new(10, 2);
	assert_eq!(backend.get_cursor_position().unwrap(), Position::ORIGIN);
}

#[test]
fn assert_cursor_position() {
	let mut backend = TestBackend::new(10, 2);
	backend.assert_cursor_position(Position::ORIGIN);
}

#[test]
fn set_cursor_position() {
	let mut backend = TestBackend::new(10, 10);
	backend.set_cursor_position(Position { x: 5, y: 5 }).unwrap();
	assert_eq!(backend.pos, (5, 5));
}
