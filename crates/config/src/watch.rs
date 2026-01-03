//! File system watcher for hot-reloading configuration files.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{ConfigError, Result};

/// Describes what kind of configuration file changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigChange {
	/// Main config file (config.kdl) changed.
	MainConfig(PathBuf),
	/// A theme file changed.
	Theme(PathBuf),
	/// A language config file changed.
	Language(PathBuf),
	/// Some other KDL file changed.
	Other(PathBuf),
}

impl ConfigChange {
	/// Returns the path of the changed file.
	pub fn path(&self) -> &Path {
		match self {
			Self::MainConfig(p) | Self::Theme(p) | Self::Language(p) | Self::Other(p) => p,
		}
	}

	/// Categorizes a file path into a config change type.
	fn from_path(path: PathBuf, config_dir: &Path) -> Option<Self> {
		if path.extension()? != "kdl" {
			return None;
		}

		let relative = path.strip_prefix(config_dir).ok();

		Some(if path.file_name()? == "config.kdl" {
			Self::MainConfig(path)
		} else if relative.is_some_and(|r| r.starts_with("themes")) {
			Self::Theme(path)
		} else if relative.is_some_and(|r| r.starts_with("languages")) {
			Self::Language(path)
		} else {
			Self::Other(path)
		})
	}
}

/// Watches a configuration directory for file changes.
///
/// Uses native OS file watching (inotify on Linux, FSEvents on macOS, etc.)
/// for efficient change detection.
pub struct ConfigWatcher {
	/// Path to the watched configuration directory.
	config_dir: PathBuf,
	/// Channel receiver for file system events.
	rx: Receiver<notify::Result<Event>>,
	/// The underlying file watcher (kept alive for RAII).
	_watcher: RecommendedWatcher,
}

impl ConfigWatcher {
	/// Creates a new watcher for the given configuration directory.
	pub fn new(config_dir: impl Into<PathBuf>) -> Result<Self> {
		let config_dir = config_dir.into();
		let (tx, rx) = mpsc::channel();

		let mut watcher = RecommendedWatcher::new(
			move |res| {
				let _ = tx.send(res);
			},
			notify::Config::default(),
		)
		.map_err(|e| ConfigError::Watch(e.to_string()))?;

		if config_dir.exists() {
			watcher
				.watch(&config_dir, RecursiveMode::Recursive)
				.map_err(|e| ConfigError::Watch(e.to_string()))?;
		}

		Ok(Self {
			config_dir,
			rx,
			_watcher: watcher,
		})
	}

	/// Polls for configuration changes without blocking.
	///
	/// Returns a list of changed configuration files since the last poll.
	/// Each file appears at most once, even if it was modified multiple times.
	pub fn poll(&self) -> Vec<ConfigChange> {
		let mut changes = Vec::new();
		let mut seen = std::collections::HashSet::new();

		while let Ok(Ok(event)) = self.rx.try_recv() {
			if !matches!(
				event.kind,
				EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
			) {
				continue;
			}

			for path in event.paths {
				if seen.insert(path.clone())
					&& let Some(change) = ConfigChange::from_path(path, &self.config_dir)
				{
					changes.push(change);
				}
			}
		}

		changes
	}

	/// Returns the configuration directory being watched.
	pub fn config_dir(&self) -> &Path {
		&self.config_dir
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn config_change_classification() {
		let config_dir = PathBuf::from("/home/user/.config/xeno");

		let main = config_dir.join("config.kdl");
		assert!(matches!(
			ConfigChange::from_path(main, &config_dir),
			Some(ConfigChange::MainConfig(_))
		));

		let theme = config_dir.join("themes/gruvbox.kdl");
		assert!(matches!(
			ConfigChange::from_path(theme, &config_dir),
			Some(ConfigChange::Theme(_))
		));

		let lang = config_dir.join("languages/rust.kdl");
		assert!(matches!(
			ConfigChange::from_path(lang, &config_dir),
			Some(ConfigChange::Language(_))
		));

		let other = config_dir.join("custom.kdl");
		assert!(matches!(
			ConfigChange::from_path(other, &config_dir),
			Some(ConfigChange::Other(_))
		));

		let non_kdl = config_dir.join("readme.txt");
		assert!(ConfigChange::from_path(non_kdl, &config_dir).is_none());
	}

	#[test]
	fn watcher_creation() {
		let tmp = TempDir::new().unwrap();
		let watcher = ConfigWatcher::new(tmp.path());
		assert!(watcher.is_ok());
	}

	#[test]
	fn watcher_detects_changes() {
		use std::time::Duration;

		let tmp = TempDir::new().unwrap();
		let config_path = tmp.path().join("config.kdl");
		fs::write(&config_path, "options {}").unwrap();

		let watcher = ConfigWatcher::new(tmp.path()).unwrap();
		std::thread::sleep(Duration::from_millis(50));

		fs::write(&config_path, "options { tab-width 4 }").unwrap();

		for _ in 0..20 {
			std::thread::sleep(Duration::from_millis(50));
			if watcher
				.poll()
				.iter()
				.any(|c| matches!(c, ConfigChange::MainConfig(_)))
			{
				return;
			}
		}
		panic!("Expected MainConfig change event");
	}
}
