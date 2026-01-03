//! Viewport stability tests using kitty harness.

mod helpers;

use std::time::Duration;

use helpers::{reset_test_file, type_chars, workspace_dir, xeno_cmd_debug_with_log};
use kitty_test_harness::{
	KeyPress, MouseButton, cleanup_test_log, create_test_log, kitty_send_keys, pause_briefly,
	read_test_log, require_kitty, run_with_timeout, send_keys, send_mouse_drag_with_steps,
	wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Creates a horizontal split (Ctrl+w s h) - top/bottom panes with horizontal separator.
fn create_horizontal_split(kitty: &kitty_test_harness::KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('s'));
	kitty_send_keys!(kitty, KeyCode::Char('h'));
	pause_briefly();
}

/// Focus the pane above (Ctrl+w f k).
fn focus_up(kitty: &kitty_test_harness::KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('f'));
	kitty_send_keys!(kitty, KeyCode::Char('k'));
	pause_briefly();
}

/// Find the row numbers where separator line characters appear.
fn find_separator_rows(clean: &str) -> Vec<usize> {
	clean
		.lines()
		.enumerate()
		.filter(|(_, line)| line.chars().all(|c| c == '─' || c == ' ') && line.contains('─'))
		.map(|(i, _)| i)
		.collect()
}

/// Finds the first line number visible in the buffer (from the gutter).
/// Returns the line number shown in the first row of content.
#[allow(dead_code, reason = "helper retained for test debugging")]
fn find_first_visible_line(clean: &str) -> Option<usize> {
	// Look for the first line that starts with a line number in the gutter
	// Format is typically "   1 " or "  10 " etc.
	for line in clean.lines() {
		let trimmed = line.trim_start();
		if let Some(num_end) = trimmed.find(' ')
			&& let Ok(num) = trimmed[..num_end].parse::<usize>()
		{
			return Some(num);
		}
	}
	None
}

/// Finds the line number at a specific screen row.
fn find_line_number_at_row(clean: &str, row: usize) -> Option<usize> {
	if let Some(line) = clean.lines().nth(row) {
		let trimmed = line.trim_start();
		if let Some(num_end) = trimmed.find(' ')
			&& let Ok(num) = trimmed[..num_end].parse::<usize>()
		{
			return Some(num);
		}
	}
	None
}

/// Tests that the viewport scroll position remains stable when an adjacent
/// split is resized, even if the cursor would go off-screen.
///
/// Scenario:
/// 1. Create top/bottom split
/// 2. Fill top buffer with numbered lines (LINE_01, LINE_02, etc.)
/// 3. Position cursor at the very last row of the visible viewport
/// 4. Drag separator UP to shrink the top buffer
/// 5. Verify the first visible line didn't change (viewport didn't scroll to chase cursor)
///
/// The bug being tested: when a vertically adjacent split is resized upward,
/// the cursor at the bottom edge would go "off-screen", and the viewport
/// would incorrectly scroll down to keep the cursor visible. This caused
/// both the cursor AND viewport to visually move up together.
#[serial_test::serial]
#[test]
fn viewport_stable_during_adjacent_split_resize() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/viewport-stability.txt";
	reset_test_file(file);
	let log_path = create_test_log();
	let log_path_clone = log_path.clone();
	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(
			&workspace_dir(),
			&xeno_cmd_debug_with_log(file, &log_path_clone),
			|kitty| {
				pause_briefly();

				// Create horizontal split (top/bottom stacked)
				create_horizontal_split(kitty);
				pause_briefly();

				// Focus the top buffer (Ctrl+w f k)
				focus_up(kitty);

				// Insert many numbered lines so we can track scroll position.
				// We need enough lines that the viewport will be scrolled and
				// the cursor can be at the bottom edge.
				send_keys(kitty, &[KeyPress::from(KeyCode::Char('i'))]);
				for i in 1..=60 {
					type_chars(kitty, &format!("LINE_{:02}", i));
					send_keys(kitty, &[KeyPress::from(KeyCode::Enter)]);
				}
				send_keys(kitty, &[KeyPress::from(KeyCode::Escape)]);
				pause_briefly();

				// Go to line 30 using gg then 29 j movements
				send_keys(kitty, &[KeyPress::from(KeyCode::Char('g'))]);
				send_keys(kitty, &[KeyPress::from(KeyCode::Char('g'))]);
				for _ in 0..29 {
					send_keys(kitty, &[KeyPress::from(KeyCode::Char('j'))]);
				}
				pause_briefly();

				// Capture screen to find the separator position
				let (_raw, clean_initial) =
					wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
						clean.contains("LINE_30")
					});

				let sep_rows = find_separator_rows(&clean_initial);
				assert!(
					!sep_rows.is_empty(),
					"Should have a separator, screen:\n{}",
					clean_initial
				);
				let separator_row = sep_rows[0];

				// Now we need to position the cursor at the very last visible row
				// before the separator. The top buffer spans rows 0 to separator_row-1.
				// First go to the top of the file
				send_keys(kitty, &[KeyPress::from(KeyCode::Char('g'))]);
				send_keys(kitty, &[KeyPress::from(KeyCode::Char('g'))]);
				pause_briefly();

				// Now move down to the last visible row (separator_row - 1).
				// This puts the cursor right at the bottom edge of the viewport.
				for _ in 0..(separator_row.saturating_sub(1)) {
					send_keys(kitty, &[KeyPress::from(KeyCode::Char('j'))]);
				}
				pause_briefly();

				// Capture screen before resize
				let (_raw, clean_before) =
					wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
						clean.contains("LINE_01")
					});

				// Find first visible line before resize (row 1 is first content after menu)
				let first_line_before = find_line_number_at_row(&clean_before, 1);
				assert!(
					first_line_before.is_some(),
					"Should find first visible line number, screen:\n{}",
					clean_before
				);
				let first_line_before = first_line_before.unwrap();

				eprintln!("Before resize:");
				eprintln!("  Separator at row: {}", separator_row);
				eprintln!("  First visible line: {}", first_line_before);
				eprintln!(
					"  Cursor should be at row {} (bottom of viewport)",
					separator_row - 1
				);
				eprintln!("Screen:\n{}", clean_before);

				// Drag the separator UP by 5 rows to shrink the top buffer
				let start_row = separator_row as u16;
				let end_row = start_row.saturating_sub(5);

				send_mouse_drag_with_steps(kitty, MouseButton::Left, 40, start_row, 40, end_row, 5);
				pause_briefly();
				pause_briefly();

				// Capture screen after resize
				let (_raw2, clean_after) =
					wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
						// Wait for screen to stabilize
						!clean.is_empty()
					});

				let sep_rows_after = find_separator_rows(&clean_after);
				assert!(
					!sep_rows_after.is_empty(),
					"Should still have separator after resize, screen:\n{}",
					clean_after
				);
				let separator_row_after = sep_rows_after[0];

				// Find first visible line after resize
				let first_line_after = find_line_number_at_row(&clean_after, 1);
				assert!(
					first_line_after.is_some(),
					"Should find first visible line after resize, screen:\n{}",
					clean_after
				);
				let first_line_after = first_line_after.unwrap();

				eprintln!("After resize:");
				eprintln!("  Separator at row: {}", separator_row_after);
				eprintln!("  First visible line: {}", first_line_after);
				eprintln!("Screen:\n{}", clean_after);

				// Verify separator moved up
				assert!(
					separator_row_after < separator_row,
					"Separator should have moved up. Before: {}, After: {}",
					separator_row,
					separator_row_after
				);

				// Print test log for debugging
				let log_content = read_test_log(&log_path_clone);
				eprintln!("Test log:\n{}", log_content.join("\n"));

				// KEY ASSERTION: First visible line should NOT have changed!
				// This is the bug we're testing for. Before the fix, the viewport
				// would scroll down to keep the cursor visible, changing the first line.
				assert_eq!(
					first_line_before, first_line_after,
					"First visible line should remain stable during resize!\n\
				 Before: LINE_{:02}, After: LINE_{:02}\n\
				 The viewport scrolled when it shouldn't have.",
					first_line_before, first_line_after
				);
			},
		);
	});
	cleanup_test_log(&log_path);
}
