//! XDG base directory utilities.
//!
//! Provides platform-specific paths following the XDG Base Directory Specification.

use std::path::PathBuf;

use crate::error::{AuthError, AuthResult};

/// Application directory name.
const APP_DIR: &str = "xeno";

/// Get the default Xeno data directory following XDG spec.
///
/// Returns XDG_DATA_HOME/xeno (~/.local/share/xeno on Linux).
pub fn default_data_dir() -> AuthResult<PathBuf> {
	let data_dir = dirs::data_dir()
		.ok_or_else(|| AuthError::Storage("could not determine XDG data directory".into()))?;
	Ok(data_dir.join(APP_DIR))
}

/// Get the default Xeno config directory following XDG spec.
///
/// Returns XDG_CONFIG_HOME/xeno (~/.config/xeno on Linux).
pub fn default_config_dir() -> AuthResult<PathBuf> {
	let config_dir = dirs::config_dir()
		.ok_or_else(|| AuthError::Storage("could not determine XDG config directory".into()))?;
	Ok(config_dir.join(APP_DIR))
}
