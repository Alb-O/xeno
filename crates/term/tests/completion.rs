use std::path::PathBuf;
use std::time::Duration;

use kitty_test_harness::{
	kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, wait_for_screen_text_clean,
	with_kitty_capture,
};
use termwiz::input::KeyCode;

const TEST_TIMEOUT: Duration = Duration::from_secs(15);

fn tome_cmd() -> String {
	env!("CARGO_BIN_EXE_tome").to_string()
}

fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[serial_test::serial]
#[test]
fn command_completion_shows_menu() {
	if !require_kitty() {
		return;
	}

	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &tome_cmd(), |kitty| {
			pause_briefly();

			// Open command palette and type 'wr'
			kitty_send_keys!(
				kitty,
				KeyCode::Char(':'),
				KeyCode::Char('w'),
				KeyCode::Char('r'),
			);

			// Should show 'write' in completion menu with its kind icon/label
			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_r, clean| {
					clean.contains("write") && clean.contains("Cmd")
				});

			assert!(
				clean.contains("write"),
				"Completion menu should show 'write'. Clean: {clean:?}"
			);
			assert!(
				clean.contains("Cmd"),
				"Completion menu should show 'Cmd' kind. Clean: {clean:?}"
			);

			// Tab to select 'write'
			kitty_send_keys!(kitty, KeyCode::Tab);

			// Command line should now have 'write'
			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_r, clean| {
					// The command line is at the bottom, it should contain ':write'
					clean.contains(":write")
				});

			assert!(
				clean.contains(":write"),
				"Command line should be filled with 'write'. Clean: {clean:?}"
			);
		});
	});
}
