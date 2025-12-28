mod helpers;

use std::time::{Duration, Instant};

use helpers::{insert_text, reset_test_file, evildoer_cmd_debug_theme, workspace_dir};
use kitty_test_harness::{
	KittyHarness, kitty_send_keys, pause_briefly, require_kitty, run_with_timeout,
	wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

fn wait_for_prompt(kitty: &KittyHarness) {
	let (_raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
		clean.contains("$") || clean.contains("%") || clean.contains(">")
	});
	assert!(
		clean.contains("$") || clean.contains("%") || clean.contains(">"),
		"terminal prompt not detected: {clean}"
	);
}

/// Tests that embedded terminal has EVILDOER_BIN exported for IPC wrappers.
#[serial_test::serial]
#[test]
fn terminal_has_evildoer_in_path() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/terminal-ipc-path.txt";
	reset_test_file(file);

	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(&workspace_dir(), &evildoer_cmd_debug_theme(file), |kitty| {
			pause_briefly();

			// Open terminal: Ctrl+w t
			kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
			kitty_send_keys!(kitty, KeyCode::Char('t'));

			// Wait for shell prompt
			wait_for_prompt(kitty);

			// Echo EVILDOER_BIN to ensure IPC wrapper path is exported
			for c in "echo $EVILDOER_BIN".chars() {
				kitty_send_keys!(kitty, KeyCode::Char(c));
			}
			kitty_send_keys!(kitty, KeyCode::Enter);

			// Wait for output containing "evildoer"
			let (_, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
					clean.contains("evildoer-")
				});

			assert!(
				clean.contains("evildoer-"),
				"EVILDOER_BIN should contain evildoer bin dir: {}",
				clean
			);
		});
	});
}

/// Tests invoking :quit from an embedded terminal via IPC.
#[serial_test::serial]
#[test]
fn terminal_ipc_quit_command() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/terminal-ipc-quit.txt";
	reset_test_file(file);

	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &evildoer_cmd_debug_theme(file), |kitty| {
			pause_briefly();

			// Open terminal: Ctrl+w t
			kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
			kitty_send_keys!(kitty, KeyCode::Char('t'));

			// Wait for shell prompt
			wait_for_prompt(kitty);

			// Type :quit - editor should exit
			for c in ":quit".chars() {
				kitty_send_keys!(kitty, KeyCode::Char(c));
			}
			kitty_send_keys!(kitty, KeyCode::Enter);
			std::thread::sleep(Duration::from_millis(100));
		});
	});
}

/// Tests a simple :write workflow from an embedded terminal.
#[serial_test::serial]
#[test]
fn terminal_ipc_write_saves_buffer() {
	if !require_kitty() {
		return;
	}

	let file = "tmp/kitty/terminal-ipc-write.txt";
	reset_test_file(file);

	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(&workspace_dir(), &evildoer_cmd_debug_theme(file), |kitty| {
			pause_briefly();
			insert_text(kitty, "ipc write workflow");
			pause_briefly();
			let _ = wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
				clean.contains("ipc write workflow")
			});

			// Open terminal: Ctrl+w t
			kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
			kitty_send_keys!(kitty, KeyCode::Char('t'));

			// Wait for shell prompt
			wait_for_prompt(kitty);

			for c in "echo FISH_CONFIG_DIR=$FISH_CONFIG_DIR".chars() {
				kitty_send_keys!(kitty, KeyCode::Char(c));
			}
			kitty_send_keys!(kitty, KeyCode::Enter);
			let _ = wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
				clean.contains("FISH_CONFIG_DIR=")
			});

			// Trigger :write via IPC
			for c in ":write".chars() {
				kitty_send_keys!(kitty, KeyCode::Char(c));
			}
			kitty_send_keys!(kitty, KeyCode::Enter);
			std::thread::sleep(Duration::from_millis(200));
			let (_raw, clean_after) = kitty.screen_text_clean();

			let path = workspace_dir().join(file);
			let deadline = Instant::now() + Duration::from_secs(3);
			let mut contents = String::new();
			while Instant::now() < deadline {
				if let Ok(read) = std::fs::read_to_string(&path) {
					contents = read;
					if contents.contains("ipc write workflow") {
						break;
					}
				}
				std::thread::sleep(Duration::from_millis(50));
			}

			assert!(
				contents.contains("ipc write workflow"),
				"expected buffer to be saved, contents: {contents:?}, screen: {clean_after:?}"
			);
		});
	});
}
