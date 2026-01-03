use std::path::PathBuf;

/// Returns the platform-specific configuration directory for xeno.
///
/// On Linux, uses `$XDG_CONFIG_HOME/xeno` or `~/.config/xeno`.
/// On other platforms, uses `~/.xeno`.
pub fn get_config_dir() -> Option<PathBuf> {
	#[cfg(target_os = "linux")]
	{
		std::env::var_os("XDG_CONFIG_HOME")
			.map(PathBuf::from)
			.or_else(|| home::home_dir().map(|h| h.join(".config")))
			.map(|p| p.join("xeno"))
	}
	#[cfg(not(target_os = "linux"))]
	{
		home::home_dir().map(|h| h.join(".xeno"))
	}
}
