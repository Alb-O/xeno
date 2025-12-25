use std::path::PathBuf;

use kitty_test_harness::{KeyPress, KittyHarness, send_keys};
use termwiz::input::KeyCode;

/// Returns the path to the tome binary.
pub fn tome_cmd() -> String {
	env!("CARGO_BIN_EXE_tome").to_string()
}

/// Returns a command to launch tome with the given file.
pub fn tome_cmd_with_file_named(name: &str) -> String {
	format!("{} {}", tome_cmd(), name)
}

/// Returns the workspace directory.
pub fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Removes the test file if it exists.
pub fn reset_test_file(name: &str) {
	let path = workspace_dir().join(name);
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
pub fn insert_text(kitty: &KittyHarness, text: &str) {
	send_keys(kitty, &[KeyPress::from(KeyCode::Char('i'))]);
	type_chars(kitty, text);
	send_keys(kitty, &[KeyPress::from(KeyCode::Escape)]);
}

/// Enters insert mode and types multiple lines of text, exiting insert mode after.
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
