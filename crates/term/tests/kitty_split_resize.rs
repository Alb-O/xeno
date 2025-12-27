mod helpers;

use std::time::Duration;

use helpers::{insert_text, reset_test_file, tome_cmd_debug_theme, workspace_dir};
use kitty_test_harness::{
	MouseButton, kitty_send_keys, pause_briefly, require_kitty, run_with_timeout,
	send_mouse_drag_with_steps, wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Creates a horizontal split (Ctrl+w s) - splits vertically (top/bottom).
fn create_horizontal_split(kitty: &kitty_test_harness::KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('s'));
	pause_briefly();
}

/// Find the row numbers where separator line characters appear.
/// Separators use a line of '─' characters for horizontal splits.
fn find_separator_rows(clean: &str) -> Vec<usize> {
	clean
		.lines()
		.enumerate()
		.filter(|(_, line)| {
			line.chars()
				.all(|c| c == '─' || c == ' ' || c == '\u{2500}')
				&& line.contains('─')
		})
		.map(|(i, _)| i)
		.collect()
}

/// Tests that dragging the OUTER separator in a nested split (A | (B | C))
/// preserves the inner separator's absolute screen position by adjusting ratios.
///
/// This catches regressions in the complex ratio-preservation logic that ensures
/// child splits maintain their visual positions when a parent is resized.
#[serial_test::serial]
#[test]
fn split_resize_outer_preserves_inner_absolute_position() {
	if !require_kitty() {
		return;
	}

	let file = "kitty-test-split-resize-outer.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &tome_cmd_debug_theme(file), |kitty| {
			pause_briefly();

			// Build A | (B | C) layout with horizontal splits
			insert_text(kitty, "AAA");
			pause_briefly();

			create_horizontal_split(kitty);
			insert_text(kitty, "BBB");
			pause_briefly();

			create_horizontal_split(kitty);
			insert_text(kitty, "CCC");
			pause_briefly();

			// Wait for all content to be visible
			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
					clean.contains("AAA") && clean.contains("BBB") && clean.contains("CCC")
				});

			let sep_rows = find_separator_rows(&clean);
			assert!(
				sep_rows.len() >= 2,
				"Should have 2 separators, found: {:?}",
				sep_rows
			);

			let outer_sep = sep_rows[0];
			let inner_sep_before = sep_rows[1];

			// Drag the OUTER separator down by 5 rows
			let start_row = outer_sep as u16;
			let end_row = start_row + 5;

			send_mouse_drag_with_steps(kitty, MouseButton::Left, 40, start_row, 40, end_row, 5);
			pause_briefly();
			pause_briefly();

			// Capture after resize
			let (_raw2, clean2) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
					clean.contains("AAA")
				});

			let sep_rows2 = find_separator_rows(&clean2);
			assert!(
				sep_rows2.len() >= 2,
				"Should still have 2 separators after resize, found: {:?}",
				sep_rows2
			);

			let outer_sep_after = sep_rows2[0];
			let inner_sep_after = sep_rows2[1];

			// Outer separator should have moved
			assert_ne!(
				outer_sep, outer_sep_after,
				"Outer separator should have moved after drag"
			);

			// Inner separator should stay at same ABSOLUTE position (or very close due to rounding)
			// The key insight: if ratio-preservation works, inner_sep_after == inner_sep_before
			assert!(
				(inner_sep_after as i32 - inner_sep_before as i32).abs() <= 1,
				"Inner separator should stay at absolute position. Before: {}, After: {}",
				inner_sep_before,
				inner_sep_after
			);

			// All buffers should still be visible
			assert!(clean2.contains("AAA"), "Should still see AAA");
			assert!(clean2.contains("BBB"), "Should still see BBB");
			assert!(clean2.contains("CCC"), "Should still see CCC");
		});
	});
}
