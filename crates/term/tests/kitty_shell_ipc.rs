//! Shell-specific IPC integration tests for bash, zsh, fish, and nushell.
//!
//! Requires `KITTY_TESTS=1`. Set `NIX_TESTS=1` to resolve shells via nix-shell,
//! otherwise falls back to PATH lookup.

mod helpers;

use std::time::{Duration, Instant};

use helpers::{
	TestShell, insert_text, require_shell, reset_test_file, tome_cmd_with_shell, workspace_dir,
};
use kitty_test_harness::{
	KittyHarness, kitty_send_keys, pause_briefly, require_kitty, run_with_timeout,
	wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Wait for shell to be ready. Detects common prompts ($, %, >) or TERMINAL mode indicator.
fn wait_for_prompt(kitty: &KittyHarness) {
	let (_raw, clean) = wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
		clean.contains("$")
			|| clean.contains("%")
			|| clean.contains(">")
			|| clean.contains("TERMINAL")
	});
	assert!(
		clean.contains("$")
			|| clean.contains("%")
			|| clean.contains(">")
			|| clean.contains("TERMINAL"),
		"terminal not ready: {clean}"
	);
	// Give shell time to fully initialize before typing
	std::thread::sleep(Duration::from_millis(300));
}

/// Opens the embedded terminal with Ctrl+w t
fn open_terminal(kitty: &KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('t'));
}

/// Types a string of characters
fn type_string(kitty: &KittyHarness, s: &str) {
	for c in s.chars() {
		kitty_send_keys!(kitty, KeyCode::Char(c));
	}
}

/// Tests that TOME_BIN is exported in the embedded terminal for a specific shell.
fn test_shell_has_tome_bin(shell: TestShell, file_suffix: &str) {
	if !require_kitty() {
		return;
	}
	if !require_shell(shell) {
		eprintln!(
			"Skipping {:?} test: shell not available (set NIX_TESTS=1 or install {})",
			shell,
			shell.binary_name()
		);
		return;
	}

	let file = format!("tmp/kitty/shell-ipc-{}-path.txt", file_suffix);
	reset_test_file(&file);

	let Some(cmd) = tome_cmd_with_shell(&file, shell) else {
		eprintln!("Could not build tome command for {:?}", shell);
		return;
	};

	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(&workspace_dir(), &cmd, |kitty| {
			pause_briefly();
			open_terminal(kitty);
			wait_for_prompt(kitty);

			// Echo TOME_BIN to verify IPC is set up
			type_string(kitty, shell.echo_tome_bin_cmd());
			kitty_send_keys!(kitty, KeyCode::Enter);

			let (_, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
					clean.contains("tome-")
				});

			assert!(
				clean.contains("tome-"),
				"{:?}: TOME_BIN should contain tome bin dir: {}",
				shell,
				clean
			);
		});
	});
}

/// Tests that :quit works from an embedded terminal for a specific shell.
fn test_shell_ipc_quit(shell: TestShell, file_suffix: &str) {
	if !require_kitty() {
		return;
	}
	if !require_shell(shell) {
		eprintln!(
			"Skipping {:?} test: shell not available (set NIX_TESTS=1 or install {})",
			shell,
			shell.binary_name()
		);
		return;
	}

	let file = format!("tmp/kitty/shell-ipc-{}-quit.txt", file_suffix);
	reset_test_file(&file);

	let Some(cmd) = tome_cmd_with_shell(&file, shell) else {
		eprintln!("Could not build tome command for {:?}", shell);
		return;
	};

	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(&workspace_dir(), &cmd, |kitty| {
			pause_briefly();
			open_terminal(kitty);
			wait_for_prompt(kitty);

			// Type the quit command (shell-specific syntax)
			type_string(kitty, shell.quit_cmd());
			kitty_send_keys!(kitty, KeyCode::Enter);
			std::thread::sleep(Duration::from_millis(100));
		});
	});
}

/// Tests that :write works from an embedded terminal for a specific shell.
fn test_shell_ipc_write(shell: TestShell, file_suffix: &str) {
	if !require_kitty() {
		return;
	}
	if !require_shell(shell) {
		eprintln!(
			"Skipping {:?} test: shell not available (set NIX_TESTS=1 or install {})",
			shell,
			shell.binary_name()
		);
		return;
	}

	let file = format!("tmp/kitty/shell-ipc-{}-write.txt", file_suffix);
	reset_test_file(&file);
	let file_clone = file.clone();

	let Some(cmd) = tome_cmd_with_shell(&file, shell) else {
		eprintln!("Could not build tome command for {:?}", shell);
		return;
	};

	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(&workspace_dir(), &cmd, |kitty| {
			pause_briefly();

			// Insert some text
			let test_content = format!("{:?} ipc write test", shell);
			insert_text(kitty, &test_content);
			pause_briefly();
			let _ = wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
				clean.contains(&test_content)
			});

			// Open terminal
			open_terminal(kitty);
			wait_for_prompt(kitty);

			// Trigger :write via IPC (shell-specific syntax)
			type_string(kitty, shell.write_cmd());
			kitty_send_keys!(kitty, KeyCode::Enter);
			std::thread::sleep(Duration::from_millis(200));
			let (_raw, clean_after) = kitty.screen_text_clean();

			// Verify file was written
			let path = workspace_dir().join(&file_clone);
			let deadline = Instant::now() + Duration::from_secs(3);
			let mut contents = String::new();
			while Instant::now() < deadline {
				if let Ok(read) = std::fs::read_to_string(&path) {
					contents = read;
					if contents.contains(&test_content) {
						break;
					}
				}
				std::thread::sleep(Duration::from_millis(50));
			}

			assert!(
				contents.contains(&test_content),
				"{:?}: expected buffer to be saved, contents: {:?}, screen: {:?}",
				shell,
				contents,
				clean_after
			);
		});
	});
}

#[serial_test::serial]
#[test]
fn bash_has_tome_bin() {
	test_shell_has_tome_bin(TestShell::Bash, "bash");
}

#[serial_test::serial]
#[test]
fn bash_ipc_quit() {
	test_shell_ipc_quit(TestShell::Bash, "bash");
}

#[serial_test::serial]
#[test]
fn bash_ipc_write() {
	test_shell_ipc_write(TestShell::Bash, "bash");
}

#[serial_test::serial]
#[test]
fn zsh_has_tome_bin() {
	test_shell_has_tome_bin(TestShell::Zsh, "zsh");
}

#[serial_test::serial]
#[test]
fn zsh_ipc_quit() {
	test_shell_ipc_quit(TestShell::Zsh, "zsh");
}

#[serial_test::serial]
#[test]
fn zsh_ipc_write() {
	test_shell_ipc_write(TestShell::Zsh, "zsh");
}

#[serial_test::serial]
#[test]
fn fish_has_tome_bin() {
	test_shell_has_tome_bin(TestShell::Fish, "fish");
}

#[serial_test::serial]
#[test]
fn fish_ipc_quit() {
	test_shell_ipc_quit(TestShell::Fish, "fish");
}

#[serial_test::serial]
#[test]
fn fish_ipc_write() {
	test_shell_ipc_write(TestShell::Fish, "fish");
}

#[serial_test::serial]
#[test]
#[ignore = "nushell IPC requires prompt and command syntax work"]
fn nushell_has_tome_bin() {
	test_shell_has_tome_bin(TestShell::Nushell, "nushell");
}

#[serial_test::serial]
#[test]
#[ignore = "nushell IPC requires prompt and command syntax work"]
fn nushell_ipc_quit() {
	test_shell_ipc_quit(TestShell::Nushell, "nushell");
}

#[serial_test::serial]
#[test]
#[ignore = "nushell IPC requires prompt and command syntax work"]
fn nushell_ipc_write() {
	test_shell_ipc_write(TestShell::Nushell, "nushell");
}
