//! Clear and clear_region tests for `TestBackend`.

use super::*;

#[test]
fn clear() {
	let mut backend = TestBackend::new(4, 2);
	let cell = Cell::new("a");
	backend.draw([(0, 0, &cell)].into_iter()).unwrap();
	backend.draw([(0, 1, &cell)].into_iter()).unwrap();
	backend.clear().unwrap();
	backend.assert_buffer_lines(["    ", "    "]);
}

#[test]
fn clear_region_all() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);

	backend.clear_region(ClearType::All).unwrap();
	backend.assert_buffer_lines(["          ", "          ", "          ", "          ", "          "]);
}

#[test]
fn clear_region_after_cursor() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);

	backend.set_cursor_position(Position { x: 3, y: 2 }).unwrap();
	backend.clear_region(ClearType::AfterCursor).unwrap();
	backend.assert_buffer_lines(["aaaaaaaaaa", "aaaaaaaaaa", "aaaa      ", "          ", "          "]);
}

#[test]
fn clear_region_before_cursor() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);

	backend.set_cursor_position(Position { x: 5, y: 3 }).unwrap();
	backend.clear_region(ClearType::BeforeCursor).unwrap();
	backend.assert_buffer_lines(["          ", "          ", "          ", "     aaaaa", "aaaaaaaaaa"]);
}

#[test]
fn clear_region_current_line() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);

	backend.set_cursor_position(Position { x: 3, y: 1 }).unwrap();
	backend.clear_region(ClearType::CurrentLine).unwrap();
	backend.assert_buffer_lines(["aaaaaaaaaa", "          ", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);
}

#[test]
fn clear_region_until_new_line() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);

	backend.set_cursor_position(Position { x: 3, y: 0 }).unwrap();
	backend.clear_region(ClearType::UntilNewLine).unwrap();
	backend.assert_buffer_lines(["aaa       ", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa", "aaaaaaaaaa"]);
}
