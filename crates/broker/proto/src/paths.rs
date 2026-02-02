//! Utilities for broker file paths and socket resolution.

use std::path::PathBuf;

/// Returns the default socket path for the xeno broker.
///
/// Prioritizes:
/// 1. `XENO_BROKER_SOCKET` environment variable.
/// 2. System runtime directory (e.g., `$XDG_RUNTIME_DIR`).
/// 3. System cache directory.
/// 4. System temp directory.
///
/// The default file name is `xeno-broker.sock`.
#[must_use]
pub fn default_socket_path() -> PathBuf {
	if let Ok(p) = std::env::var("XENO_BROKER_SOCKET") {
		return PathBuf::from(p);
	}

	let runtime_dir = dirs::runtime_dir()
		.or_else(dirs::cache_dir)
		.unwrap_or_else(std::env::temp_dir);

	runtime_dir.join("xeno-broker.sock")
}
