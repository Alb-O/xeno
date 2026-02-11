//! Tests for `Backend::append_lines` implementation.

use super::*;

#[test]
fn append_lines_not_at_last_line() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	backend.set_cursor_position(Position::ORIGIN).unwrap();

	// If the cursor is not at the last line in the terminal the addition of a
	// newline simply moves the cursor down and to the right

	backend.append_lines(1).unwrap();
	backend.assert_cursor_position(Position { x: 1, y: 1 });

	backend.append_lines(1).unwrap();
	backend.assert_cursor_position(Position { x: 2, y: 2 });

	backend.append_lines(1).unwrap();
	backend.assert_cursor_position(Position { x: 3, y: 3 });

	backend.append_lines(1).unwrap();
	backend.assert_cursor_position(Position { x: 4, y: 4 });

	// As such the buffer should remain unchanged
	backend.assert_buffer_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);
	backend.assert_scrollback_empty();
}

#[test]
fn append_lines_at_last_line() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	// If the cursor is at the last line in the terminal the addition of a
	// newline will scroll the contents of the buffer
	backend.set_cursor_position(Position { x: 0, y: 4 }).unwrap();

	backend.append_lines(1).unwrap();

	backend.assert_buffer_lines(["bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee", "          "]);
	backend.assert_scrollback_lines(["aaaaaaaaaa"]);

	// It also moves the cursor to the right, as is common of the behaviour of
	// terminals in raw-mode
	backend.assert_cursor_position(Position { x: 1, y: 4 });
}

#[test]
fn append_multiple_lines_not_at_last_line() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	backend.set_cursor_position(Position::ORIGIN).unwrap();

	// If the cursor is not at the last line in the terminal the addition of multiple
	// newlines simply moves the cursor n lines down and to the right by 1

	backend.append_lines(4).unwrap();
	backend.assert_cursor_position(Position { x: 1, y: 4 });

	// As such the buffer should remain unchanged
	backend.assert_buffer_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);
	backend.assert_scrollback_empty();
}

#[test]
fn append_multiple_lines_past_last_line() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	backend.set_cursor_position(Position { x: 0, y: 3 }).unwrap();

	backend.append_lines(3).unwrap();
	backend.assert_cursor_position(Position { x: 1, y: 4 });

	backend.assert_buffer_lines(["cccccccccc", "dddddddddd", "eeeeeeeeee", "          ", "          "]);
	backend.assert_scrollback_lines(["aaaaaaaaaa", "bbbbbbbbbb"]);
}

#[test]
fn append_multiple_lines_where_cursor_at_end_appends_height_lines() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	backend.set_cursor_position(Position { x: 0, y: 4 }).unwrap();

	backend.append_lines(5).unwrap();
	backend.assert_cursor_position(Position { x: 1, y: 4 });

	backend.assert_buffer_lines(["          ", "          ", "          ", "          ", "          "]);
	backend.assert_scrollback_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);
}

#[test]
fn append_multiple_lines_where_cursor_appends_height_lines() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	backend.set_cursor_position(Position::ORIGIN).unwrap();

	backend.append_lines(5).unwrap();
	backend.assert_cursor_position(Position { x: 1, y: 4 });

	backend.assert_buffer_lines(["bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee", "          "]);
	backend.assert_scrollback_lines(["aaaaaaaaaa"]);
}

#[test]
fn append_multiple_lines_where_cursor_at_end_appends_more_than_height_lines() {
	let mut backend = TestBackend::with_lines(["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee"]);

	backend.set_cursor_position(Position { x: 0, y: 4 }).unwrap();

	backend.append_lines(8).unwrap();
	backend.assert_cursor_position(Position { x: 1, y: 4 });

	backend.assert_buffer_lines(["          ", "          ", "          ", "          ", "          "]);
	backend.assert_scrollback_lines([
		"aaaaaaaaaa",
		"bbbbbbbbbb",
		"cccccccccc",
		"dddddddddd",
		"eeeeeeeeee",
		"          ",
		"          ",
		"          ",
	]);
}

#[test]
fn append_lines_truncates_beyond_u16_max() -> Result<()> {
	let mut backend = TestBackend::new(10, 5);

	// Fill the scrollback with 65535 + 10 lines.
	let row_count = u16::MAX as usize + 10;
	for row in 0..=row_count {
		if row > 4 {
			backend.set_cursor_position(Position { x: 0, y: 4 })?;
			backend.append_lines(1)?;
		}
		let cells = format!("{row:>10}").chars().map(Cell::from).collect::<Vec<_>>();
		let content = cells.iter().enumerate().map(|(column, cell)| (column as u16, 4.min(row) as u16, cell));
		backend.draw(content)?;
	}

	// check that the buffer contains the last 5 lines appended
	backend.assert_buffer_lines(["     65541", "     65542", "     65543", "     65544", "     65545"]);

	// TODO: ideally this should be something like:
	//     let lines = (6..=65545).map(|row| format!("{row:>10}"));
	//     backend.assert_scrollback_lines(lines);
	// but there's some truncation happening in Buffer::with_lines that needs to be fixed
	assert_eq!(
		Buffer {
			area: Rect::new(0, 0, 10, 5),
			content: backend.scrollback.content[0..10 * 5].to_vec(),
		},
		Buffer::with_lines(["         6", "         7", "         8", "         9", "        10",]),
		"first 5 lines of scrollback should have been truncated"
	);

	assert_eq!(
		Buffer {
			area: Rect::new(0, 0, 10, 5),
			content: backend.scrollback.content[10 * 65530..10 * 65535].to_vec(),
		},
		Buffer::with_lines(["     65536", "     65537", "     65538", "     65539", "     65540",]),
		"last 5 lines of scrollback should have been appended"
	);

	// These checks come after the content checks as otherwise we won't see the failing content
	// when these checks fail.
	// Make sure the scrollback is the right size.
	assert_eq!(backend.scrollback.area.width, 10);
	assert_eq!(backend.scrollback.area.height, 65535);
	assert_eq!(backend.scrollback.content.len(), 10 * 65535);
	Ok(())
}
