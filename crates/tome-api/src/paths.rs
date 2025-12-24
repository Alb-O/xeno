use std::path::PathBuf;

pub fn get_config_dir() -> Option<PathBuf> {
	#[cfg(target_os = "linux")]
	{
		std::env::var_os("XDG_CONFIG_HOME")
			.map(PathBuf::from)
			.or_else(|| home::home_dir().map(|h| h.join(".config")))
			.map(|p| p.join("tome"))
	}
	#[cfg(not(target_os = "linux"))]
	{
		home::home_dir().map(|h| h.join(".tome"))
	}
}
