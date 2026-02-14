//! Stale syntax regression test for L-tier (>1MB) files.
//!
//! Replays a recorded session against `tmp/miniaudio.h` that searches,
//! scrolls, selects, deletes, and undoes — exercising background syntax
//! parsing on a large file where stale highlights have historically appeared.

use std::time::Duration;

use kitty_test_harness::{
	ReplayTiming, fg_color_at_text, parse_recording, pause_briefly, replay, require_kitty, run_with_timeout, wait_for_screen_text_clean, with_kitty_capture,
};

use crate::helpers::{workspace_dir, xeno_cmd_debug_theme};

const TEST_TIMEOUT: Duration = Duration::from_secs(30);
const KEY_DELAY: Duration = Duration::from_millis(50);

const RECORDING: &str = include_str!("kitty_ltier_stale_syntax.xsession");

/// Debug theme comment foreground: `$gray-mid` = `#646464`.
const COMMENT_COLOR: (u8, u8, u8) = (0x64, 0x64, 0x64);

/// Debug theme default foreground: `$white` = `#FFFFFF`.
const DEFAULT_FG: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);

/// Searches for "Misc", scrolls with pagedown/shift-pagedown, deletes a
/// selection, then undoes — verifying the viewport lands on the expected
/// content with correct comment highlighting on an L-tier file.
#[serial_test::serial]
#[test]
fn ltier_stale_syntax_after_search_and_undo() {
	if !require_kitty() {
		return;
	}

	let file = "../../tmp/miniaudio.h";
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &xeno_cmd_debug_theme(file), |kitty| {
			wait_for_screen_text_clean(kitty, Duration::from_secs(10), |_raw, clean| clean.contains("miniaudio.h"));

			pause_briefly();

			let events = parse_recording(RECORDING);
			replay(kitty, &events, ReplayTiming::per_key(KEY_DELAY));

			pause_briefly();
			pause_briefly();

			let (raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| clean.contains("Miscellaneous Notes"));

			// Find the row by clean text, then grab the corresponding raw line.
			// extract_row_colors_parsed can't distinguish gutter-fg from comment-fg
			// (both use $gray-mid), so we walk the raw ANSI to check the color
			// active at the actual text position.
			let row = clean
				.lines()
				.position(|line| line.contains("Miscellaneous Notes"))
				.expect("'Miscellaneous Notes' should be on screen");

			let raw_row = raw.lines().nth(row).expect("raw output should have matching row");

			let fg_at_text = fg_color_at_text(raw_row, "Miscellaneous");
			assert_eq!(
				fg_at_text,
				Some(COMMENT_COLOR),
				"'Miscellaneous Notes' should be rendered with comment color {COMMENT_COLOR:?}, got {fg_at_text:?}\nraw line: {raw_row:?}"
			);

			// MA_VERSION_MINOR is a preprocessor identifier — it must have a
			// syntax-highlighted color (not default white). When syntax is stale,
			// deleted->undone text falls back to white, catches the bug even when
			// the comment line happens to be correctly colored.
			let minor_row = clean
				.lines()
				.position(|line| line.contains("MA_VERSION_MINOR"))
				.expect("'MA_VERSION_MINOR' should be on screen");
			let minor_raw = raw.lines().nth(minor_row).expect("raw output should have matching row");
			let minor_fg = fg_color_at_text(minor_raw, "MA_VERSION_MINOR");
			assert_ne!(
				minor_fg,
				Some(DEFAULT_FG),
				"'MA_VERSION_MINOR' should have syntax highlighting (not default white)\nraw line: {minor_raw:?}"
			);
		});
	});
}
