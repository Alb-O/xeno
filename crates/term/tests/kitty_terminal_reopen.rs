use std::path::PathBuf;
use std::time::Duration;

use kitty_test_harness::{
	kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, wait_for_screen_text_clean,
	with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

fn tome_cmd() -> String {
	env!("CARGO_BIN_EXE_tome").to_string()
}

fn tome_cmd_with_file_named(name: &str) -> String {
	format!("{} {}", tome_cmd(), name)
}

fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn reset_test_file(name: &str) {
	let path = workspace_dir().join(name);
	let _ = std::fs::remove_file(&path);
}

#[serial_test::serial]
#[test]
fn terminal_reopens_after_exit_and_is_ready() {
	if !require_kitty() {
		return;
	}

	let file = "kitty-test-terminal-reopen.txt";
	reset_test_file(file);

	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &tome_cmd_with_file_named(file), |kitty| {
			// Allow boot + prewarm.
			pause_briefly();
			pause_briefly();

			// Open terminal and run a command.
			kitty_send_keys!(kitty, (KeyCode::Char('t'), Modifiers::CTRL));
			kitty_send_keys!(
				kitty,
				KeyCode::Char('e'),
				KeyCode::Char('c'),
				KeyCode::Char('h'),
				KeyCode::Char('o'),
				KeyCode::Char(' '),
				KeyCode::Char('o'),
				KeyCode::Char('n'),
				KeyCode::Char('e'),
				KeyCode::Enter
			);

			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
					clean.contains("one")
				});
			assert!(clean.contains("one"), "clean: {clean:?}");

			// Exit the shell. Editor should auto-close the panel.
			kitty_send_keys!(
				kitty,
				KeyCode::Char('e'),
				KeyCode::Char('x'),
				KeyCode::Char('i'),
				KeyCode::Char('t'),
				KeyCode::Enter
			);

			// Confirm we are back in editor by typing a marker.
			kitty_send_keys!(kitty, KeyCode::Char('i'), KeyCode::Char('Z'));
			kitty_send_keys!(kitty, KeyCode::Escape);
			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
					clean.contains("Z")
				});
			assert!(clean.contains("Z"), "clean: {clean:?}");

			// Re-open terminal and run another command without waiting.
			kitty_send_keys!(kitty, (KeyCode::Char('t'), Modifiers::CTRL));
			kitty_send_keys!(
				kitty,
				KeyCode::Char('e'),
				KeyCode::Char('c'),
				KeyCode::Char('h'),
				KeyCode::Char('o'),
				KeyCode::Char(' '),
				KeyCode::Char('t'),
				KeyCode::Char('w'),
				KeyCode::Char('o'),
				KeyCode::Enter
			);

			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
					clean.contains("two")
				});
			assert!(clean.contains("two"), "clean: {clean:?}");
		});
	});
}
