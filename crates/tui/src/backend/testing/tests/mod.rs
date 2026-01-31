//! Tests for the `TestBackend` implementation.

use super::*;

mod append_lines;
mod clear;
mod cursor;
#[cfg(feature = "scrolling-regions")]
mod scrolling_regions;

#[test]
fn new() {
	assert_eq!(
		TestBackend::new(10, 2),
		TestBackend {
			buffer: Buffer::with_lines(["          "; 2]),
			scrollback: Buffer::empty(Rect::new(0, 0, 10, 0)),
			cursor: false,
			pos: (0, 0),
		}
	);
}

#[test]
fn test_buffer_view() {
	let buffer = Buffer::with_lines(["aaaa"; 2]);
	assert_eq!(buffer_view(&buffer), "\"aaaa\"\n\"aaaa\"\n");
}

#[test]
fn buffer_view_with_overwrites() {
	let multi_byte_char = "👨‍👩‍👧‍👦"; // renders 2 wide
	let buffer = Buffer::with_lines([multi_byte_char]);
	assert_eq!(
		buffer_view(&buffer),
		format!(
			r#""{multi_byte_char}" Hidden by multi-width symbols: [(1, " ")]
"#,
		)
	);
}

#[test]
fn buffer() {
	let backend = TestBackend::new(10, 2);
	backend.assert_buffer_lines(["          "; 2]);
}

#[test]
fn resize() {
	let mut backend = TestBackend::new(10, 2);
	backend.resize(5, 5);
	backend.assert_buffer_lines(["     "; 5]);
}

#[test]
fn assert_buffer() {
	let backend = TestBackend::new(10, 2);
	backend.assert_buffer_lines(["          "; 2]);
}

#[test]
#[should_panic = "buffer contents not equal"]
fn assert_buffer_panics() {
	let backend = TestBackend::new(10, 2);
	backend.assert_buffer_lines(["aaaaaaaaaa"; 2]);
}

#[test]
#[should_panic = "assertion `left == right` failed"]
fn assert_scrollback_panics() {
	let backend = TestBackend::new(10, 2);
	backend.assert_scrollback_lines(["aaaaaaaaaa"; 2]);
}

#[test]
fn display() {
	let backend = TestBackend::new(10, 2);
	assert_eq!(format!("{backend}"), "\"          \"\n\"          \"\n");
}

#[test]
fn draw() {
	let mut backend = TestBackend::new(10, 2);
	let cell = Cell::new("a");
	backend.draw([(0, 0, &cell)].into_iter()).unwrap();
	backend.draw([(0, 1, &cell)].into_iter()).unwrap();
	backend.assert_buffer_lines(["a         "; 2]);
}

#[test]
fn size() {
	let backend = TestBackend::new(10, 2);
	assert_eq!(backend.size().unwrap(), Size::new(10, 2));
}

#[test]
fn flush() {
	let mut backend = TestBackend::new(10, 2);
	backend.flush().unwrap();
}
