use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

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

/// Returns an absolute path without hitting the filesystem.
///
/// Absolute inputs are returned as-is. Relative inputs are joined against the
/// current working directory.
pub fn fast_abs(path: &Path) -> PathBuf {
	if path.is_absolute() {
		path.to_path_buf()
	} else {
		std::env::current_dir().unwrap_or_default().join(path)
	}
}

/// Lexically normalizes a path without touching the filesystem.
///
/// This collapses `.` segments, resolves `..` where possible, and preserves
/// absolute/prefix semantics without following symlinks.
pub fn normalize_lexical(path: &Path) -> PathBuf {
	let mut prefix: Option<OsString> = None;
	let mut absolute = false;
	let mut stack: Vec<OsString> = Vec::new();

	for component in path.components() {
		match component {
			Component::Prefix(prefix_component) => {
				prefix = Some(prefix_component.as_os_str().to_os_string());
			}
			Component::RootDir => {
				absolute = true;
				stack.clear();
			}
			Component::CurDir => {}
			Component::ParentDir => {
				if stack.last().is_some_and(|last| last != "..") {
					stack.pop();
				} else if !absolute {
					stack.push(OsString::from(".."));
				}
			}
			Component::Normal(segment) => stack.push(segment.to_os_string()),
		}
	}

	let mut out = PathBuf::new();
	if let Some(prefix) = prefix {
		out.push(prefix);
	}
	if absolute {
		out.push(Path::new(std::path::MAIN_SEPARATOR_STR));
	}
	for part in stack {
		out.push(part);
	}

	if out.as_os_str().is_empty() { PathBuf::from(".") } else { out }
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::{fast_abs, normalize_lexical};

	#[test]
	fn normalize_lexical_collapses_dot_and_parent_segments() {
		let normalized = normalize_lexical(Path::new("a/./b/../c"));
		assert_eq!(normalized, Path::new("a/c"));
	}

	#[test]
	fn normalize_lexical_stops_parent_at_root() {
		let normalized = normalize_lexical(Path::new("/a/../../b"));
		assert_eq!(normalized, Path::new("/b"));
	}

	#[test]
	fn normalize_lexical_preserves_leading_parent_on_relative_paths() {
		let normalized = normalize_lexical(Path::new("../../a/../b"));
		assert_eq!(normalized, Path::new("../../b"));
	}

	#[test]
	fn fast_abs_returns_absolute_path() {
		let rel = Path::new("src/lib.rs");
		assert!(fast_abs(rel).is_absolute());
	}
}
