//! Multi-cursor selection tests using kitty harness.

mod helpers;

use std::time::Duration;

use helpers::{evildoer_cmd_debug_theme, insert_lines, reset_test_file, workspace_dir};
use kitty_test_harness::{
	kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, wait_for_clean_contains,
	wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::KeyCode;

const TEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Tests that typing in insert mode affects all cursors in a multi-selection.
/// This is a critical multi-cursor feature that has historically had bugs.
#[serial_test::serial]
#[test]
fn insert_mode_types_at_all_cursors() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/multiselect-insert.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &evildoer_cmd_debug_theme(file), |kitty| {
			pause_briefly();
			insert_lines(kitty, &["one", "two", "three"]);
			pause_briefly();

			// Select all, split per line, and enter insert mode to type 'X'.
			kitty_send_keys!(kitty, KeyCode::Char('%'));
			kitty_send_keys!(kitty, (KeyCode::Char('s'), termwiz::input::Modifiers::ALT));
			kitty_send_keys!(kitty, KeyCode::Char('i'));
			kitty_send_keys!(kitty, KeyCode::Char('X'));
			kitty_send_keys!(kitty, KeyCode::Escape);

			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
					clean.contains("Xone") && clean.contains("Xtwo") && clean.contains("Xthree")
				});

			assert!(clean.contains("Xone"), "clean: {clean:?}");
			assert!(clean.contains("Xtwo"), "clean: {clean:?}");
			assert!(clean.contains("Xthree"), "clean: {clean:?}");
		});
	});
}

/// Tests that 'a' (append) inserts after each cursor position across multiple selections.
/// Verifies correct cursor offset handling with backward selections from split-lines.
#[serial_test::serial]
#[test]
fn insert_a_appends_after_each_cursor_across_selections() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/multiselect-insert-after.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &evildoer_cmd_debug_theme(file), |kitty| {
			pause_briefly();
			insert_lines(kitty, &["one", "two", "three"]);
			pause_briefly();

			let clean_initial = wait_for_clean_contains(kitty, Duration::from_secs(3), "three");
			assert!(
				clean_initial.contains("one"),
				"clean_initial: {clean_initial:?}"
			);

			// Per-line cursors (split lines creates backward selections 4..0, 8..4 etc, heads at start)
			// so 'a' should append after the first character of each line.
			kitty_send_keys!(kitty, KeyCode::Char('%'));
			kitty_send_keys!(kitty, (KeyCode::Char('s'), termwiz::input::Modifiers::ALT));
			kitty_send_keys!(kitty, KeyCode::Char('a'));
			kitty_send_keys!(kitty, KeyCode::Char('+'));
			kitty_send_keys!(kitty, KeyCode::Escape);

			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
					clean.contains("t+wo") && clean.contains("t+hree")
				});

			assert!(clean.contains("t+wo"), "clean: {clean:?}");
			assert!(clean.contains("t+hree"), "clean: {clean:?}");
			assert!(
				!clean.contains("+one"),
				"append-after should not insert at the start, clean: {clean:?}"
			);
		});
	});
}
