//! Grammar loading and search path configuration.
//!
//! Grammars are compiled tree-sitter parsers loaded from shared libraries.
//! This module handles locating and loading grammar files.

use std::path::{Path, PathBuf};

use thiserror::Error;
use tree_house::tree_sitter::Grammar;

/// Errors that can occur when loading a grammar.
#[derive(Error, Debug)]
pub enum GrammarError {
	#[error("grammar not found: {0}")]
	NotFound(String),

	#[error("failed to load grammar library: {0}")]
	LoadError(String),

	#[error("grammar library missing language function: {0}")]
	MissingSymbol(String),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
}

/// Loads a grammar by name from the search paths.
///
/// Searches all configured grammar directories for a matching shared library.
pub fn load_grammar(name: &str) -> Result<Grammar, GrammarError> {
	let lib_name = grammar_library_name(name);

	for path in grammar_search_paths() {
		let lib_path = path.join(&lib_name);

		if lib_path.exists() {
			return load_grammar_from_path(&lib_path, name);
		}
	}

	Err(GrammarError::NotFound(name.to_string()))
}

/// Loads a grammar from a specific library path.
fn load_grammar_from_path(path: &Path, name: &str) -> Result<Grammar, GrammarError> {
	// SAFETY: Loading a tree-sitter grammar from a dynamic library.
	// The library must contain a valid tree-sitter language function.
	unsafe {
		Grammar::new(name, path)
			.map_err(|e| GrammarError::LoadError(format!("{}: {}", path.display(), e)))
	}
}

/// Returns the platform-specific library name for a grammar.
fn grammar_library_name(name: &str) -> String {
	let safe_name = name.replace('-', "_");
	#[cfg(target_os = "macos")]
	{
		format!("lib{safe_name}.dylib")
	}
	#[cfg(target_os = "windows")]
	{
		format!("{safe_name}.dll")
	}
	#[cfg(not(any(target_os = "macos", target_os = "windows")))]
	{
		format!("lib{safe_name}.so")
	}
}

/// Source for loading a grammar.
#[derive(Debug, Clone)]
pub enum GrammarSource {
	/// Load from a shared library at the given path.
	Library(PathBuf),
	/// Use a pre-compiled grammar (future: for bundled grammars).
	Builtin(&'static str),
}

/// Returns runtime directories where grammars are searched.
/// Order: TOME_RUNTIME env, user config dir, system/bundled dir.
pub fn grammar_search_paths() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	// Development: check TOME_RUNTIME env var first
	if let Ok(runtime) = std::env::var("TOME_RUNTIME") {
		dirs.push(PathBuf::from(runtime).join("grammars"));
	}

	// User config directory: ~/.config/tome/grammars/
	if let Some(config_dir) = config_dir() {
		dirs.push(config_dir.join("tome").join("grammars"));
	}

	if let Some(data_dir) = data_local_dir() {
		dirs.push(data_dir.join("tome").join("grammars"));
	}

	// Bundled grammars relative to executable
	if let Ok(exe_path) = std::env::current_exe() {
		if let Some(exe_dir) = exe_path.parent() {
			dirs.push(exe_dir.join("grammars"));
			// Also check ../share/tome/grammars for installed packages
			dirs.push(
				exe_dir
					.join("..")
					.join("share")
					.join("tome")
					.join("grammars"),
			);
		}
	}

	dirs
}

/// Returns directories to search for query files.
pub fn query_search_paths() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	// TOME_RUNTIME env var (development)
	if let Ok(runtime) = std::env::var("TOME_RUNTIME") {
		dirs.push(PathBuf::from(runtime).join("queries"));
	}

	if let Some(config) = config_dir() {
		dirs.push(config.join("tome").join("queries"));
	}

	if let Ok(exe) = std::env::current_exe() {
		if let Some(dir) = exe.parent() {
			dirs.push(dir.join("queries"));
			dirs.push(dir.join("..").join("share").join("tome").join("queries"));
		}
	}

	dirs
}

// Minimal platform-specific directory helpers
fn config_dir() -> Option<PathBuf> {
	#[cfg(unix)]
	{
		std::env::var_os("XDG_CONFIG_HOME")
			.map(PathBuf::from)
			.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
	}
	#[cfg(windows)]
	{
		std::env::var_os("APPDATA").map(PathBuf::from)
	}
	#[cfg(not(any(unix, windows)))]
	{
		None
	}
}

fn data_local_dir() -> Option<PathBuf> {
	#[cfg(unix)]
	{
		std::env::var_os("XDG_DATA_HOME")
			.map(PathBuf::from)
			.or_else(|| {
				std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
			})
	}
	#[cfg(windows)]
	{
		std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
	}
	#[cfg(not(any(unix, windows)))]
	{
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_search_paths_not_empty() {
		// Should have at least the exe-relative path
		let dirs = grammar_search_paths();
		assert!(!dirs.is_empty());
	}
}
