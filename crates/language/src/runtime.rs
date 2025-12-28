//! Runtime initialization and seeding.
//!
//! This module handles copying the embedded query files and themes to the user's
//! runtime directory (`~/.local/share/evildoer/`) on first use.

use std::path::Path;
use std::{fs, io};

use include_dir::{Dir, include_dir};

use crate::grammar::runtime_dir;

/// Embedded query files from `runtime/language/queries/`.
static QUERIES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../runtime/language/queries");

/// Embedded theme files from `runtime/themes/`.
static THEMES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../runtime/themes");

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
		log::info!("Seeding runtime queries to {}", queries_dir.display());
		seed_queries(&queries_dir)?;
	}

	// Seed theme files
	let themes_dir = runtime.join("themes");
	if !themes_dir.exists() {
		log::info!("Seeding runtime themes to {}", themes_dir.display());
		seed_themes(&themes_dir)?;
	}

	Ok(())
}

/// Copies embedded query files to the target directory.
fn seed_queries(target: &Path) -> io::Result<()> {
	extract_dir(&QUERIES_DIR, target)
}

/// Copies embedded theme files to the target directory.
fn seed_themes(target: &Path) -> io::Result<()> {
	extract_dir(&THEMES_DIR, target)
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
	log::info!("Re-seeding runtime queries to {}", queries_dir.display());
	seed_queries(&queries_dir)?;

	// Re-seed themes
	let themes_dir = runtime.join("themes");
	if themes_dir.exists() {
		fs::remove_dir_all(&themes_dir)?;
	}
	log::info!("Re-seeding runtime themes to {}", themes_dir.display());
	seed_themes(&themes_dir)?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_queries_embedded() {
		let dirs: Vec<_> = QUERIES_DIR.dirs().collect();
		assert!(!dirs.is_empty(), "Should have language directories");

		let rust_dir = QUERIES_DIR.get_dir("rust");
		assert!(rust_dir.is_some(), "Should have rust queries directory");

		let rust = rust_dir.unwrap();
		let files: Vec<_> = rust.files().collect();
		assert!(!files.is_empty(), "Rust should have query files");

		let has_highlights = files
			.iter()
			.any(|f| f.path().file_name().is_some_and(|n| n == "highlights.scm"));
		assert!(has_highlights, "Should have highlights.scm");
	}

	#[test]
	fn test_themes_embedded() {
		let files: Vec<_> = THEMES_DIR.files().collect();
		assert!(!files.is_empty(), "Should have theme files");

		let has_gruvbox = files
			.iter()
			.any(|f| f.path().file_name().is_some_and(|n| n == "gruvbox.kdl"));
		assert!(has_gruvbox, "Should have gruvbox.kdl");

		let has_one_dark = files
			.iter()
			.any(|f| f.path().file_name().is_some_and(|n| n == "one_dark.kdl"));
		assert!(has_one_dark, "Should have one_dark.kdl");
	}
}
