use std::path::PathBuf;

/// Application directory name.
const APP_DIR: &str = "xeno";

/// Returns the platform-specific configuration directory for xeno.
///
/// Uses XDG base directories: `$XDG_CONFIG_HOME/xeno` (~/.config/xeno on Linux).
pub fn get_config_dir() -> Option<PathBuf> {
	dirs::config_dir().map(|p| p.join(APP_DIR))
}

/// Returns the platform-specific data directory for xeno.
///
/// Uses XDG base directories: `$XDG_DATA_HOME/xeno` (~/.local/share/xeno on Linux).
pub fn get_data_dir() -> Option<PathBuf> {
	dirs::data_dir().map(|p| p.join(APP_DIR))
}

/// Returns the platform-specific cache directory for xeno.
///
/// Uses XDG base directories: `$XDG_CACHE_HOME/xeno` (~/.cache/xeno on Linux).
pub fn get_cache_dir() -> Option<PathBuf> {
	dirs::cache_dir().map(|p| p.join(APP_DIR))
}
