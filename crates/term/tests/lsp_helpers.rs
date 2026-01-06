//! Shared utilities for LSP integration tests.

use std::path::PathBuf;
use std::time::Duration;

use kitty_test_harness::{pause_briefly, require_kitty, wait_for_screen_text_clean, KittyHarness};

/// Check if LSP tests should run.
///
/// Returns `true` if both `LSP_TESTS` and `KITTY_TESTS` env vars are set
/// and the kitty harness is available.
pub fn require_lsp_tests() -> bool {
	if std::env::var("LSP_TESTS").is_err() {
		eprintln!("Skipping LSP test (set LSP_TESTS=1 to run)");
		return false;
	}
	require_kitty()
}

/// Returns path to LSP test fixtures.
pub fn fixtures_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp")
}

/// Returns the workspace directory for test execution.
pub fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Wait for LSP to initialize (diagnostics to appear).
///
/// rust-analyzer needs time to index the project. This function waits
/// for a reasonable initialization period for rust-analyzer to start.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn wait_for_lsp_ready(kitty: &KittyHarness, timeout: Duration) {
	// Give the LSP some time to start up
	pause_briefly();
	pause_briefly();

	// Wait for the screen to be ready, then give rust-analyzer time to index
	let _ = wait_for_screen_text_clean(kitty, Duration::from_millis(500), |_raw, _clean| true);

	// Wait for rust-analyzer to settle - it needs time to index the project
	// This is the primary waiting mechanism for LSP readiness
	let wait_time = timeout.min(Duration::from_secs(10));
	std::thread::sleep(wait_time);
}

/// Returns command to launch xeno with a file.
///
/// Uses the debug theme for consistent rendering during tests.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn xeno_cmd_with_file(file: &str) -> String {
	format!("{} --theme debug {}", xeno_cmd(), file)
}

/// Returns the path to the xeno binary.
pub fn xeno_cmd() -> String {
	env!("CARGO_BIN_EXE_xeno").to_string()
}
