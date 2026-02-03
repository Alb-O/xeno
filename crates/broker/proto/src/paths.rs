//! Utilities for broker file paths and socket resolution.

use std::path::PathBuf;

/// Returns the default socket path for the xeno broker.
///
/// Prioritizes writable directories to ensure the broker can bind its IPC socket
/// even in restricted or "homeless" environments (e.g. Nix builds, containers).
///
/// # Resolution Order
///
/// 1. `XENO_BROKER_SOCKET` environment variable.
/// 2. System runtime directory (e.g., `$XDG_RUNTIME_DIR`).
/// 3. System temp directory (e.g., `/tmp`).
///
/// The default file name is `xeno-broker.sock`.
#[must_use]
pub fn default_socket_path() -> PathBuf {
	if let Ok(p) = std::env::var("XENO_BROKER_SOCKET") {
		return PathBuf::from(p);
	}

	// Try runtime dir first (XDG_RUNTIME_DIR), falling back to /tmp if unwritable.
	dirs::runtime_dir()
		.filter(|p| std::fs::create_dir_all(p).is_ok())
		.unwrap_or_else(std::env::temp_dir)
		.join("xeno-broker.sock")
}
