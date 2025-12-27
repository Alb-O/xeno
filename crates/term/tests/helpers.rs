use std::path::{Path, PathBuf};
use std::process::Command;

use kitty_test_harness::{KeyPress, KittyHarness, send_keys};
use termwiz::input::KeyCode;

/// Shell types supported for IPC testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "test helper used by individual test files")]
pub enum TestShell {
	Bash,
	Zsh,
	Fish,
	Nushell,
}

impl TestShell {
	/// Returns the shell binary name.
	pub fn binary_name(&self) -> &'static str {
		match self {
			TestShell::Bash => "bash",
			TestShell::Zsh => "zsh",
			TestShell::Fish => "fish",
			TestShell::Nushell => "nu",
		}
	}

	/// Returns the command to echo TOME_BIN in this shell's syntax.
	pub fn echo_tome_bin_cmd(&self) -> &'static str {
		match self {
			TestShell::Bash | TestShell::Zsh | TestShell::Fish => "echo $TOME_BIN",
			TestShell::Nushell => "echo $env.TOME_BIN",
		}
	}

	/// Returns the :write command in this shell's syntax.
	pub fn write_cmd(&self) -> &'static str {
		match self {
			TestShell::Nushell => "^$env.TOME_BIN/:write",
			_ => ":write",
		}
	}

	/// Returns the :quit command in this shell's syntax.
	pub fn quit_cmd(&self) -> &'static str {
		match self {
			TestShell::Nushell => "^$env.TOME_BIN/:quit",
			_ => ":quit",
		}
	}
}

/// Resolves shell binary path. Uses nix-shell when `NIX_TESTS` is set,
/// otherwise falls back to PATH lookup.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn resolve_shell(shell: TestShell) -> Option<PathBuf> {
	let binary = shell.binary_name();

	if std::env::var("NIX_TESTS").is_ok() {
		if let Some(path) = resolve_shell_via_nix(binary) {
			return Some(path);
		}
	}

	find_in_path(binary)
}

/// Uses nix-shell to resolve a shell binary path.
fn resolve_shell_via_nix(binary: &str) -> Option<PathBuf> {
	let nix_file = workspace_dir().join("tests/shell-deps.nix");

	if !nix_file.exists() {
		return None;
	}

	let output = Command::new("nix-shell")
		.arg(&nix_file)
		.arg("--run")
		.arg(format!("command -v {}", binary))
		.output()
		.ok()?;

	if output.status.success() {
		let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
		if !path.is_empty() {
			return Some(PathBuf::from(path));
		}
	}

	None
}

/// Finds a binary in PATH.
fn find_in_path(binary: &str) -> Option<PathBuf> {
	let path_var = std::env::var_os("PATH")?;
	for dir in std::env::split_paths(&path_var) {
		let candidate = dir.join(binary);
		if candidate.is_file() {
			return Some(candidate);
		}
	}
	None
}

/// Returns true if the shell is available for testing.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn require_shell(shell: TestShell) -> bool {
	resolve_shell(shell).is_some()
}

/// Returns a tome command with a specific SHELL environment variable set.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn tome_cmd_with_shell(file: &str, shell: TestShell) -> Option<String> {
	let shell_path = resolve_shell(shell)?;
	Some(format!(
		"SHELL={} {} --theme debug {}",
		shell_path.display(),
		tome_cmd(),
		file
	))
}

/// Returns the path to the tome binary.
pub fn tome_cmd() -> String {
	env!("CARGO_BIN_EXE_tome").to_string()
}

/// Returns a command to launch tome with the given file.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn tome_cmd_with_file_named(name: &str) -> String {
	format!("{} {}", tome_cmd(), name)
}

/// Returns a command to launch tome with the debug theme and a given file.
///
/// The debug theme uses predictable RGB values that are easy to test against.
/// See `tome_theme::themes::debug::colors` for the exact values.
pub fn tome_cmd_debug_theme(name: &str) -> String {
	format!("{} --theme debug {}", tome_cmd(), name)
}

/// Returns a command to launch tome with the debug theme and test logging enabled.
///
/// The `log_path` should be a file path where debug logs will be written.
/// Use `kitty_test_harness::create_test_log()` to create the log file.
pub fn tome_cmd_debug_with_log(name: &str, log_path: &Path) -> String {
	format!(
		"TOME_TEST_LOG={} {} --theme debug {}",
		log_path.display(),
		tome_cmd(),
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
