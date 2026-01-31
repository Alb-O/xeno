//! Tests for Buffer diff functionality.

use super::*;

/// Helper to run diff_into and return (x, y, &Cell) tuples for assertion compatibility.
fn run_diff<'a>(prev: &Buffer, next: &'a Buffer) -> Vec<(u16, u16, &'a Cell)> {
	let mut updates = Vec::new();
	prev.diff_into(next, &mut updates);
	updates
		.iter()
		.map(|u| (u.x, u.y, &next.content[u.idx]))
		.collect()
}

#[test]
fn diff_empty_empty() {
	let area = Rect::new(0, 0, 40, 40);
	let prev = Buffer::empty(area);
	let next = Buffer::empty(area);
	let diff = run_diff(&prev, &next);
	assert_eq!(diff, []);
}

#[test]
fn diff_empty_filled() {
	let area = Rect::new(0, 0, 40, 40);
	let prev = Buffer::empty(area);
	let next = Buffer::filled(area, Cell::new("a"));
	let diff = run_diff(&prev, &next);
	assert_eq!(diff.len(), 40 * 40);
}

#[test]
fn diff_filled_filled() {
	let area = Rect::new(0, 0, 40, 40);
	let prev = Buffer::filled(area, Cell::new("a"));
	let next = Buffer::filled(area, Cell::new("a"));
	let diff = run_diff(&prev, &next);
	assert_eq!(diff, []);
}

#[test]
fn diff_single_width() {
	let prev = Buffer::with_lines([
		"          ",
		"┌Title─┐  ",
		"│      │  ",
		"│      │  ",
		"└──────┘  ",
	]);
	let next = Buffer::with_lines([
		"          ",
		"┌TITLE─┐  ",
		"│      │  ",
		"│      │  ",
		"└──────┘  ",
	]);
	let diff = run_diff(&prev, &next);
	assert_eq!(
		diff,
		[
			(2, 1, &Cell::new("I")),
			(3, 1, &Cell::new("T")),
			(4, 1, &Cell::new("L")),
			(5, 1, &Cell::new("E")),
		]
	);
}

#[test]
fn diff_multi_width() {
	#[rustfmt::skip]
        let prev = Buffer::with_lines([
            "┌Title─┐  ",
            "└──────┘  ",
        ]);
	#[rustfmt::skip]
        let next = Buffer::with_lines([
            "┌称号──┐  ",
            "└──────┘  ",
        ]);
	let diff = run_diff(&prev, &next);
	assert_eq!(
		diff,
		[
			(1, 0, &Cell::new("称")),
			// Skipped "i"
			(3, 0, &Cell::new("号")),
			// Skipped "l"
			(5, 0, &Cell::new("─")),
		]
	);
}

#[test]
fn diff_multi_width_offset() {
	let prev = Buffer::with_lines(["┌称号──┐"]);
	let next = Buffer::with_lines(["┌─称号─┐"]);

	let diff = run_diff(&prev, &next);
	assert_eq!(
		diff,
		[
			(1, 0, &Cell::new("─")),
			(2, 0, &Cell::new("称")),
			(4, 0, &Cell::new("号")),
		]
	);
}

#[test]
fn diff_skip() {
	let prev = Buffer::with_lines(["123"]);
	let mut next = Buffer::with_lines(["456"]);
	for i in 1..3 {
		next.content[i].set_skip(true);
	}

	let diff = run_diff(&prev, &next);
	assert_eq!(diff, [(0, 0, &Cell::new("4"))],);
}

#[test]
fn diff_clears_trailing_cell_for_wide_grapheme() {
	// Reproduce: write "ab", then overwrite with a wide emoji like "⌨️"
	let prev = Buffer::with_lines(["ab"]); // width 2 area inferred
	assert_eq!(prev.area.width, 2);

	let mut next = Buffer::with_lines(["  "]); // start with blanks
	next.set_string(0, 0, "⌨️", Style::new());

	// The next buffer contains a wide grapheme occupying cell 0 and implicitly cell 1.
	// The debug formatting shows the hidden trailing space.
	let expected_next = Buffer::with_lines(["⌨️"]);
	assert_eq!(next, expected_next);

	// The diff should include an update for (0,0) to draw the emoji. Depending on
	// terminal behavior, it may or may not be necessary to explicitly clear (1,0).
	// At minimum, ensure the first cell is updated and nothing incorrect is emitted.
	let diff = run_diff(&prev, &next);
	assert!(
		diff.iter()
			.any(|(x, y, c)| *x == 0 && *y == 0 && c.symbol() == "⌨️")
	);
	// And it should explicitly clear the trailing cell (1,0) to avoid leftovers on terminals
	// that don't automatically clear the following cell for wide characters.
	assert!(
		diff.iter()
			.any(|(x, y, c)| *x == 1 && *y == 0 && c.symbol() == " ")
	);
}

#[test]
fn diff_vs16_wide_glyph_clears_trailing_cell() {
	let prev = Buffer::with_lines(["ab"]);
	assert_eq!(prev.area.width, 2);

	let mut next = Buffer::with_lines(["  "]);
	next.set_string(0, 0, "❤️", Style::new());

	let mut updates = Vec::new();
	prev.diff_into(&next, &mut updates);

	// Head cell at (0,0) must be present.
	assert!(updates.iter().any(|u| u.x == 0 && u.y == 0 && u.idx == 0));
	// Trailing cell at (1,0) must be explicitly cleared.
	assert!(updates.iter().any(|u| u.x == 1 && u.y == 0 && u.idx == 1));
}
