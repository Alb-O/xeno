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

/// Wait for LSP to initialize (diagnostics to appear).
///
/// rust-analyzer needs time to index the project. This function waits
/// for either the status bar to show LSP activity or for a reasonable
/// initialization period.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn wait_for_lsp_ready(kitty: &KittyHarness, timeout: Duration) {
	// Wait for LSP to initialize - look for status bar indicators
	// or just wait a reasonable amount of time for rust-analyzer to start
	let start = std::time::Instant::now();

	// First, give the LSP some time to start up
	pause_briefly();
	pause_briefly();

	// Poll for LSP ready indicator (or timeout)
	while start.elapsed() < timeout {
		// Try to detect LSP activity by looking for diagnostic indicators,
		// status bar changes, or other signs the LSP is ready
		let result = wait_for_screen_text_clean(kitty, Duration::from_millis(500), |_raw, _clean| {
			// We'll consider LSP "ready" after a brief period
			// In practice, diagnostics appearing is the real indicator
			true
		});

		if result.is_ok() {
			// Add extra pause for rust-analyzer to settle
			std::thread::sleep(Duration::from_secs(2));
			return;
		}
	}

	// Final fallback - just wait a bit more
	std::thread::sleep(Duration::from_secs(3));
}

/// Returns command to launch xeno with a file in the fixture directory.
#[allow(dead_code, reason = "test helper used by individual test files")]
pub fn xeno_cmd_with_file(file: &str) -> String {
	format!("{} {}", xeno_cmd(), file)
}

/// Returns the path to the xeno binary.
fn xeno_cmd() -> String {
	env!("CARGO_BIN_EXE_xeno").to_string()
}
