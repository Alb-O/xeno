use std::path::PathBuf;

/// Returns the platform-specific configuration directory for evildoer.
///
/// On Linux, uses `$XDG_CONFIG_HOME/evildoer` or `~/.config/evildoer`.
/// On other platforms, uses `~/.evildoer`.
pub fn get_config_dir() -> Option<PathBuf> {
	#[cfg(target_os = "linux")]
	{
		std::env::var_os("XDG_CONFIG_HOME")
			.map(PathBuf::from)
			.or_else(|| home::home_dir().map(|h| h.join(".config")))
			.map(|p| p.join("evildoer"))
	}
	#[cfg(not(target_os = "linux"))]
	{
		home::home_dir().map(|h| h.join(".evildoer"))
	}
}
