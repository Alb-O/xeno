//! Runtime initialization and seeding.
//!
//! This module handles copying the embedded query files and themes to the user's
//! runtime directory (`~/.local/share/xeno/`) on first use.
//!
//! # Version Tracking
//!
//! A `.version` file in the runtime directory tracks which xeno version seeded
//! the assets. When xeno updates, users are notified to run `:reseed` if their
//! local assets are outdated.

use std::path::Path;
use std::{fs, io};

use tracing::info;
use xeno_runtime_data::include_dir::Dir;

use crate::grammar::runtime_dir;

/// Current xeno version used for runtime asset versioning.
const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Filename for the version tracking file.
const VERSION_FILE: &str = ".version";

/// Status of runtime assets after initialization check.
#[derive(Debug, Clone)]
pub enum RuntimeStatus {
	/// Runtime is up to date with current xeno version.
	UpToDate,
	/// Runtime assets are outdated and should be reseeded.
	Outdated {
		/// Version that seeded the local runtime.
		local: String,
		/// Current xeno version.
		expected: String,
	},
	/// Fresh installation, assets were just seeded.
	FreshInstall,
}

/// Ensures the runtime directory exists and is populated with query files and themes.
///
/// This should be called once during editor startup. It copies the embedded
/// query files to `~/.local/share/xeno/queries/` and themes to
/// `~/.local/share/xeno/themes/` if they don't already exist.
///
/// Returns the runtime status indicating whether assets are up to date, outdated,
/// or freshly installed. Pre-versioning installations (assets exist but no `.version`
/// file) are reported as outdated with version "pre-0.5".
pub fn ensure_runtime() -> io::Result<RuntimeStatus> {
	let runtime = runtime_dir();
	let version_path = runtime.join(VERSION_FILE);
	let assets_exist = runtime.join("themes").exists() || runtime.join("queries").exists();

	if !version_path.exists() {
		if assets_exist {
			return Ok(RuntimeStatus::Outdated {
				local: "pre-0.5".to_string(),
				expected: RUNTIME_VERSION.to_string(),
			});
		}
		eprintln!("Initializing runtime assets...");
		seed_all(&runtime)?;
		write_version_file(&runtime)?;
		return Ok(RuntimeStatus::FreshInstall);
	}

	seed_missing_dirs(&runtime)?;
	check_runtime_version(&runtime)
}

/// Seeds any missing asset directories. Called when version file exists but
/// directories were manually deleted.
fn seed_missing_dirs(runtime: &Path) -> io::Result<()> {
	let queries_dir = runtime.join("queries");
	if !queries_dir.exists() {
		info!(path = %queries_dir.display(), "Seeding runtime queries");
		seed_queries(&queries_dir)?;
	}

	let themes_dir = runtime.join("themes");
	if !themes_dir.exists() {
		info!(path = %themes_dir.display(), "Seeding runtime themes");
		seed_themes(&themes_dir)?;
	}

	Ok(())
}

/// Seeds all runtime assets (queries and themes) and writes version file.
fn seed_all(runtime: &Path) -> io::Result<()> {
	let queries_dir = runtime.join("queries");
	info!(path = %queries_dir.display(), "Seeding runtime queries");
	seed_queries(&queries_dir)?;

	let themes_dir = runtime.join("themes");
	info!(path = %themes_dir.display(), "Seeding runtime themes");
	seed_themes(&themes_dir)?;

	Ok(())
}

/// Writes the version file with the current xeno version.
fn write_version_file(runtime: &Path) -> io::Result<()> {
	let version_path = runtime.join(VERSION_FILE);
	fs::write(version_path, RUNTIME_VERSION)
}

/// Checks if the local runtime version matches the current xeno version.
fn check_runtime_version(runtime: &Path) -> io::Result<RuntimeStatus> {
	let local = fs::read_to_string(runtime.join(VERSION_FILE))?;
	let local = local.trim();

	Ok(if local == RUNTIME_VERSION {
		RuntimeStatus::UpToDate
	} else {
		RuntimeStatus::Outdated {
			local: local.to_string(),
			expected: RUNTIME_VERSION.to_string(),
		}
	})
}

/// Copies embedded query files to the target directory.
fn seed_queries(target: &Path) -> io::Result<()> {
	extract_dir(xeno_runtime_data::queries::root(), target)
}

/// Copies embedded theme files to the target directory.
fn seed_themes(target: &Path) -> io::Result<()> {
	extract_dir(xeno_runtime_data::themes::root(), target)
}

/// Recursively extracts an embedded directory to the filesystem.
fn extract_dir(dir: &Dir<'_>, target: &Path) -> io::Result<()> {
	fs::create_dir_all(target)?;

	for file in dir.files() {
		let dest = target.join(file.path().file_name().unwrap());
		fs::write(&dest, file.contents())?;
	}

	for subdir in dir.dirs() {
		let subdir_name = subdir.path().file_name().unwrap();
		extract_dir(subdir, &target.join(subdir_name))?;
	}

	Ok(())
}

/// Forces re-seeding of runtime files, overwriting existing ones.
///
/// Removes existing query and theme directories, re-extracts them from
/// embedded assets, and updates the version file to the current xeno version.
pub fn reseed_runtime() -> io::Result<()> {
	let runtime = runtime_dir();

	let queries_dir = runtime.join("queries");
	if queries_dir.exists() {
		fs::remove_dir_all(&queries_dir)?;
	}
	info!(path = %queries_dir.display(), "Re-seeding runtime queries");
	seed_queries(&queries_dir)?;

	let themes_dir = runtime.join("themes");
	if themes_dir.exists() {
		fs::remove_dir_all(&themes_dir)?;
	}
	info!(path = %themes_dir.display(), "Re-seeding runtime themes");
	seed_themes(&themes_dir)?;

	write_version_file(&runtime)?;
	info!(version = RUNTIME_VERSION, "Updated runtime version file");

	Ok(())
}

#[cfg(test)]
mod tests {
	#[test]
	fn test_queries_embedded() {
		let languages: Vec<_> = xeno_runtime_data::queries::languages().collect();
		assert!(!languages.is_empty(), "Should have language directories");
		assert!(languages.contains(&"rust"), "Should have rust queries");

		let highlights = xeno_runtime_data::queries::get_str("rust", "highlights");
		assert!(highlights.is_some(), "Should have rust highlights.scm");
	}

	#[test]
	fn test_themes_embedded() {
		let themes: Vec<_> = xeno_runtime_data::themes::list().collect();
		assert!(!themes.is_empty(), "Should have theme files");
		assert!(themes.contains(&"gruvbox.kdl"), "Should have gruvbox.kdl");
		assert!(themes.contains(&"one_dark.kdl"), "Should have one_dark.kdl");
	}
}
