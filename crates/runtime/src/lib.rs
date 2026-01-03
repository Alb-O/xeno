//! Embedded runtime assets for xeno.
//!
//! Provides compile-time embedded access to:
//! - Tree-sitter queries (highlights, indents, textobjects, etc.)
//! - Theme definitions (KDL files)
//! - Language configuration (grammars.kdl, languages.kdl, etc.)

use include_dir::{Dir, include_dir};

pub extern crate include_dir;

/// Embedded tree-sitter query files.
static QUERIES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/language/queries");
/// Embedded theme KDL files.
static THEMES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/themes");
/// Embedded language configuration files.
static LANGUAGE_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/language");

/// Tree-sitter query files organized by language.
pub mod queries {
	use include_dir::{Dir, File};

	use super::QUERIES_DIR;

	/// Returns the queries directory for a language, or `None` if it doesn't exist.
	pub fn get_dir(language: &str) -> Option<&'static Dir<'static>> {
		QUERIES_DIR.get_dir(language)
	}

	/// Returns a query file (e.g., `get_file("rust", "highlights")` for highlights.scm).
	pub fn get_file(language: &str, query_type: &str) -> Option<&'static File<'static>> {
		let dir = QUERIES_DIR.get_dir(language)?;
		dir.get_file(format!("{language}/{query_type}.scm"))
	}

	/// Returns query file contents as a string.
	pub fn get_str(language: &str, query_type: &str) -> Option<&'static str> {
		get_file(language, query_type).and_then(|f| f.contents_utf8())
	}

	/// Lists all available language directories.
	pub fn languages() -> impl Iterator<Item = &'static str> {
		QUERIES_DIR
			.dirs()
			.filter_map(|d| d.path().file_name())
			.filter_map(|n| n.to_str())
	}

	/// Returns the root queries directory for extraction/seeding.
	pub fn root() -> &'static Dir<'static> {
		&QUERIES_DIR
	}
}

/// Theme definition files.
pub mod themes {
	use include_dir::File;

	use super::THEMES_DIR;

	/// Returns a theme file by filename (e.g., "gruvbox.kdl").
	pub fn get_file(name: &str) -> Option<&'static File<'static>> {
		THEMES_DIR.get_file(name)
	}

	/// Returns theme file contents as a string.
	pub fn get_str(name: &str) -> Option<&'static str> {
		get_file(name).and_then(|f| f.contents_utf8())
	}

	/// Lists all available theme files.
	pub fn list() -> impl Iterator<Item = &'static str> {
		THEMES_DIR
			.files()
			.filter_map(|f| f.path().file_name())
			.filter_map(|n| n.to_str())
	}

	/// Returns the root themes directory for extraction/seeding.
	pub fn root() -> &'static include_dir::Dir<'static> {
		&THEMES_DIR
	}
}

/// Language configuration files.
pub mod language {
	use include_dir::File;

	use super::LANGUAGE_DIR;

	/// Returns the embedded grammars.kdl content.
	pub fn grammars_kdl() -> &'static str {
		LANGUAGE_DIR
			.get_file("grammars.kdl")
			.and_then(|f| f.contents_utf8())
			.expect("grammars.kdl missing")
	}

	/// Returns the embedded languages.kdl content.
	pub fn languages_kdl() -> &'static str {
		LANGUAGE_DIR
			.get_file("languages.kdl")
			.and_then(|f| f.contents_utf8())
			.expect("languages.kdl missing")
	}

	/// Returns the embedded lsp.kdl content.
	pub fn lsp_kdl() -> &'static str {
		LANGUAGE_DIR
			.get_file("lsp.kdl")
			.and_then(|f| f.contents_utf8())
			.expect("lsp.kdl missing")
	}

	/// Returns the embedded formatters.kdl content.
	pub fn formatters_kdl() -> &'static str {
		LANGUAGE_DIR
			.get_file("formatters.kdl")
			.and_then(|f| f.contents_utf8())
			.expect("formatters.kdl missing")
	}

	/// Returns the embedded debuggers.kdl content.
	pub fn debuggers_kdl() -> &'static str {
		LANGUAGE_DIR
			.get_file("debuggers.kdl")
			.and_then(|f| f.contents_utf8())
			.expect("debuggers.kdl missing")
	}

	/// Returns any language config file by name.
	pub fn get_file(name: &str) -> Option<&'static File<'static>> {
		LANGUAGE_DIR.get_file(name)
	}

	/// Returns language config file contents as a string.
	pub fn get_str(name: &str) -> Option<&'static str> {
		get_file(name).and_then(|f| f.contents_utf8())
	}

	/// Returns the root language directory for extraction/seeding.
	pub fn root() -> &'static include_dir::Dir<'static> {
		&LANGUAGE_DIR
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn queries_rust_highlights() {
		assert!(queries::get_str("rust", "highlights").is_some());
	}

	#[test]
	fn queries_languages_list() {
		assert!(queries::languages().any(|l| l == "rust"));
	}

	#[test]
	fn themes_gruvbox() {
		assert!(themes::get_str("gruvbox.kdl").is_some());
	}

	#[test]
	fn themes_list() {
		assert!(themes::list().any(|t| t == "gruvbox.kdl"));
	}

	#[test]
	fn language_grammars_kdl() {
		assert!(language::grammars_kdl().contains("rust"));
	}

	#[test]
	fn language_languages_kdl() {
		assert!(!language::languages_kdl().is_empty());
	}
}
