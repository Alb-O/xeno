//! Syntax undo-memory test for L-tier (>1MB) files.
//!
//! Replays a recorded session against `tmp/miniaudio.h` that searches,
//! scrolls, selects, deletes, and undoes — then asserts that correct syntax
//! highlighting is present *immediately* after undo, with no polling for a
//! background reparse. The undo step should restore the previously known-good
//! tree, so highlights must be available without delay.

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

/// After undo on an L-tier file, the syntax tree from before the edit should
/// be restored immediately — no background reparse needed. This test verifies
/// that both comment and preprocessor tokens have correct highlighting on the
/// *first* capture after the undo completes.
#[serial_test::serial]
#[test]
fn ltier_syntax_immediately_correct_after_undo() {
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

			// Wait for the viewport text to appear (buffer content is
			// immediate after undo), but do NOT poll for correct colors.
			// We expect colors to already be correct on the first capture
			// that contains the expected text.
			let (raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(10), |_raw, clean| {
				clean.contains("Miscellaneous Notes") && clean.contains("MA_VERSION_MINOR")
			});

			// Assert correct syntax highlighting on the FIRST capture that
			// showed the expected text — no second chance / polling for colors.
			let row = clean
				.lines()
				.position(|line| line.contains("Miscellaneous Notes"))
				.expect("'Miscellaneous Notes' should be on screen");
			let raw_row = raw.lines().nth(row).expect("raw output should have matching row");
			let fg_at_text = fg_color_at_text(raw_row, "Miscellaneous");
			assert_eq!(
				fg_at_text,
				Some(COMMENT_COLOR),
				"'Miscellaneous Notes' must have comment color on first capture (undo should restore tree); got {fg_at_text:?}\nraw line: {raw_row:?}"
			);

			let minor_row = clean
				.lines()
				.position(|line| line.contains("MA_VERSION_MINOR"))
				.expect("'MA_VERSION_MINOR' should be on screen");
			let minor_raw = raw.lines().nth(minor_row).expect("raw output should have matching row");
			let minor_fg = fg_color_at_text(minor_raw, "MA_VERSION_MINOR");
			assert_ne!(
				minor_fg,
				Some(DEFAULT_FG),
				"'MA_VERSION_MINOR' must have syntax highlighting on first capture (not default white)\nraw line: {minor_raw:?}"
			);
		});
	});
}
