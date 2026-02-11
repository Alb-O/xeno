//! Split junction rendering tests using kitty harness.

use std::time::Duration;

use kitty_test_harness::{kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, wait_for_screen_text_clean, with_kitty_capture};
use termwiz::input::{KeyCode, Modifiers};

use crate::helpers::{insert_text, reset_test_file, workspace_dir, xeno_cmd_debug_theme};

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Creates a horizontal split (Ctrl+w s h) - top/bottom panes with horizontal separator.
fn create_horizontal_split(kitty: &kitty_test_harness::KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('s'));
	kitty_send_keys!(kitty, KeyCode::Char('h'));
	pause_briefly();
}

/// Creates a vertical split (Ctrl+w s v) - left/right panes with vertical separator.
fn create_vertical_split(kitty: &kitty_test_harness::KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('s'));
	kitty_send_keys!(kitty, KeyCode::Char('v'));
	pause_briefly();
}

/// Tests that T-junctions (├) appear where horizontal separators meet a vertical separator.
///
/// Layout: A | (B over C)
/// Expected:
/// ```
///    A    │    B
///         ├─────
///         │    C
/// ```
/// The ├ character should appear because the horizontal extends RIGHT from the vertical.
#[serial_test::serial]
#[test]
fn split_left_pane_with_right_horizontal() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/split-junction-opens-right.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &xeno_cmd_debug_theme(file), |kitty| {
			pause_briefly();

			// Create A | (B over C)
			insert_text(kitty, "AAAA");
			pause_briefly();

			// Vertical split: A | new
			create_vertical_split(kitty);
			insert_text(kitty, "BBBB");
			pause_briefly();

			// Horizontal split in right pane: A | (B over C)
			create_horizontal_split(kitty);
			insert_text(kitty, "CCCC");
			pause_briefly();

			// Wait for all content
			let (_raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
				clean.contains("AAAA") && clean.contains("BBBB") && clean.contains("CCCC")
			});

			// Check for junction characters
			let has_left_t = clean.contains('├');
			let has_h_line = clean.contains('─');
			let has_v_line = clean.contains('│');

			assert!(has_h_line, "Should have horizontal separator ─\n{clean}");
			assert!(has_v_line, "Should have vertical separator │\n{clean}");
			assert!(has_left_t, "Should have ├ junction (opens right) where horizontal meets vertical\n{clean}");
		});
	});
}

/// Tests T-junction (┬) where vertical separator starts at a horizontal separator.
///
/// Layout: C over (A | B)  - horizontal on top, vertical below it
/// Expected:
/// ```
///         C
///    ─────┬─────
///    A    │    B
/// ```
/// The ┬ character indicates the vertical starts below the horizontal.
#[serial_test::serial]
#[test]
fn split_vertical_starts_at_horizontal() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/split-junction-v-starts-h.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &xeno_cmd_debug_theme(file), |kitty| {
			pause_briefly();

			// Create layout: C over (A | B)
			// Step 1: Horizontal split first (focus goes to bottom)
			create_horizontal_split(kitty);
			pause_briefly();

			// Step 2: Vertical split in bottom pane (A | B)
			insert_text(kitty, "AAA");
			pause_briefly();
			create_vertical_split(kitty);
			insert_text(kitty, "BBB");
			pause_briefly();

			// Step 3: Focus to top pane (Ctrl+w k) and add content
			kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
			kitty_send_keys!(kitty, KeyCode::Char('k'));
			pause_briefly();
			insert_text(kitty, "CCC");
			pause_briefly();

			// Wait for all content
			let (_raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
				clean.contains("AAA") && clean.contains("BBB") && clean.contains("CCC")
			});

			// Check for junction characters - should have ┬ where vertical starts below horizontal
			let has_down_t = clean.contains('┬');
			let has_h_line = clean.contains('─');
			let has_v_line = clean.contains('│');

			assert!(has_h_line, "Should have horizontal separator ─\n{clean}");
			assert!(has_v_line, "Should have vertical separator │\n{clean}");
			assert!(has_down_t, "Should have ┬ junction where vertical starts below horizontal\n{clean}");
		});
	});
}

/// Tests that a T-junction (┬) appears when vertical separator meets horizontal from below.
///
/// Layout: C over (A|B)
/// Expected:
/// ```
///       C
///    ───┬───
///    A  │  B
/// ```
#[serial_test::serial]
#[test]
fn split_t_junction_down() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/split-junction-t-down.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &xeno_cmd_debug_theme(file), |kitty| {
			pause_briefly();

			// Create C over (A|B) by:
			// 1. Horizontal split (focus goes to bottom)
			// 2. Vertical split in bottom
			// Result: C (top) over (A|B) with junction ┬ where separator meets horizontal from below

			create_horizontal_split(kitty);
			pause_briefly();

			create_vertical_split(kitty);
			pause_briefly();

			// Insert text to verify layout
			insert_text(kitty, "BOT");
			pause_briefly();

			// Wait for layout to stabilize
			let (_raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| clean.contains('│') && clean.contains('─'));

			// Should have ┬ junction (down T) where vertical meets horizontal from below
			let has_down_t = clean.contains('┬');
			let has_h_line = clean.contains('─');
			let has_v_line = clean.contains('│');

			assert!(has_h_line, "Should have horizontal separator ─\n{clean}");
			assert!(has_v_line, "Should have vertical separator │\n{clean}");
			assert!(has_down_t, "Should have down-T junction ┬ where vertical meets horizontal from below\n{clean}");
		});
	});
}
