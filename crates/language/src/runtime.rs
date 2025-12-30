//! Runtime initialization and seeding.
//!
//! This module handles copying the embedded query files and themes to the user's
//! runtime directory (`~/.local/share/evildoer/`) on first use.

use std::path::Path;
use std::{fs, io};

use evildoer_runtime::include_dir::Dir;
use tracing::info;

use crate::grammar::runtime_dir;

/// Ensures the runtime directory exists and is populated with query files and themes.
///
/// This should be called once during editor startup. It copies the embedded
/// query files to `~/.local/share/evildoer/queries/` and themes to
/// `~/.local/share/evildoer/themes/` if they don't already exist.
pub fn ensure_runtime() -> io::Result<()> {
	let runtime = runtime_dir();

	// Seed query files
	let queries_dir = runtime.join("queries");
	if !queries_dir.exists() {
		info!(path = %queries_dir.display(), "Seeding runtime queries");
		seed_queries(&queries_dir)?;
	}

	// Seed theme files
	let themes_dir = runtime.join("themes");
	if !themes_dir.exists() {
		info!(path = %themes_dir.display(), "Seeding runtime themes");
		seed_themes(&themes_dir)?;
	}

	Ok(())
}

/// Copies embedded query files to the target directory.
fn seed_queries(target: &Path) -> io::Result<()> {
	extract_dir(evildoer_runtime::queries::root(), target)
}

/// Copies embedded theme files to the target directory.
fn seed_themes(target: &Path) -> io::Result<()> {
	extract_dir(evildoer_runtime::themes::root(), target)
}

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
pub fn reseed_runtime() -> io::Result<()> {
	let runtime = runtime_dir();

	// Re-seed queries
	let queries_dir = runtime.join("queries");
	if queries_dir.exists() {
		fs::remove_dir_all(&queries_dir)?;
	}
	info!(path = %queries_dir.display(), "Re-seeding runtime queries");
	seed_queries(&queries_dir)?;

	// Re-seed themes
	let themes_dir = runtime.join("themes");
	if themes_dir.exists() {
		fs::remove_dir_all(&themes_dir)?;
	}
	info!(path = %themes_dir.display(), "Re-seeding runtime themes");
	seed_themes(&themes_dir)?;

	Ok(())
}

#[cfg(test)]
mod tests {
	#[test]
	fn test_queries_embedded() {
		let languages: Vec<_> = evildoer_runtime::queries::languages().collect();
		assert!(!languages.is_empty(), "Should have language directories");
		assert!(languages.contains(&"rust"), "Should have rust queries");

		let highlights = evildoer_runtime::queries::get_str("rust", "highlights");
		assert!(highlights.is_some(), "Should have rust highlights.scm");
	}

	#[test]
	fn test_themes_embedded() {
		let themes: Vec<_> = evildoer_runtime::themes::list().collect();
		assert!(!themes.is_empty(), "Should have theme files");
		assert!(themes.contains(&"gruvbox.kdl"), "Should have gruvbox.kdl");
		assert!(themes.contains(&"one_dark.kdl"), "Should have one_dark.kdl");
	}
}
