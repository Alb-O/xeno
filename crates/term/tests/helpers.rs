//! Shared test utilities for kitty harness tests.

use std::path::{Path, PathBuf};

use kitty_test_harness::{send_keys, KeyPress, KittyHarness};
use termwiz::input::KeyCode;

/// Returns the path to the evildoer binary.
pub fn evildoer_cmd() -> String {
	env!("CARGO_BIN_EXE_evil").to_string()
}

/// Returns a command to launch evildoer with the given file.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn evildoer_cmd_with_file_named(name: &str) -> String {
	format!("{} {}", evildoer_cmd(), name)
}

/// Returns a command to launch evildoer with the debug theme and a given file.
///
/// The debug theme uses predictable RGB values that are easy to test against.
/// See `evildoer_theme::themes::debug::colors` for the exact values.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn evildoer_cmd_debug_theme(name: &str) -> String {
	format!("{} --theme debug {}", evildoer_cmd(), name)
}

/// Returns a command to launch evildoer with the debug theme and test logging enabled.
///
/// The `log_path` should be a file path where debug logs will be written.
/// Use `kitty_test_harness::create_test_log()` to create the log file.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn evildoer_cmd_debug_with_log(name: &str, log_path: &Path) -> String {
	format!(
		"EVILDOER_TEST_LOG={} {} --theme debug {}",
		log_path.display(),
		evildoer_cmd(),
		name
	)
}

/// Returns the workspace directory.
pub fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Removes the test file if it exists.
pub fn reset_test_file(name: &str) {
	let path = workspace_dir().join(name);
	if let Some(parent) = path.parent() {
		let _ = std::fs::create_dir_all(parent);
	}
	let _ = std::fs::remove_file(&path);
}

/// Types a series of characters in insert mode.
pub fn type_chars(kitty: &KittyHarness, chars: &str) {
	for ch in chars.chars() {
		if ch == '\n' {
			send_keys(kitty, &[KeyPress::from(KeyCode::Enter)]);
		} else {
			send_keys(kitty, &[KeyPress::from(KeyCode::Char(ch))]);
		}
	}
}

/// Enters insert mode, types text, and exits insert mode.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn insert_text(kitty: &KittyHarness, text: &str) {
	send_keys(kitty, &[KeyPress::from(KeyCode::Char('i'))]);
	type_chars(kitty, text);
	send_keys(kitty, &[KeyPress::from(KeyCode::Escape)]);
}

/// Enters insert mode and types multiple lines of text, exiting insert mode after.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn insert_lines(kitty: &KittyHarness, lines: &[&str]) {
	send_keys(kitty, &[KeyPress::from(KeyCode::Char('i'))]);
	for (i, line) in lines.iter().enumerate() {
		type_chars(kitty, line);
		if i < lines.len() - 1 {
			send_keys(kitty, &[KeyPress::from(KeyCode::Enter)]);
		}
	}
	send_keys(kitty, &[KeyPress::from(KeyCode::Enter)]);
	send_keys(kitty, &[KeyPress::from(KeyCode::Escape)]);
}
