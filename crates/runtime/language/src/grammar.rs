//! Grammar loading and search path configuration.
//!
//! Grammars are compiled tree-sitter parsers loaded from shared libraries.
//! This module handles locating and loading grammar files.
//!
//! # Runtime Directory
//!
//! Xeno stores all runtime data (grammars, queries) in `~/.local/share/xeno/`.
//! On first use, query files are seeded from the embedded runtime, and grammars
//! are built on-demand when a language is first opened.
//!
//! Helix's runtime directories are checked as a fallback for users who already
//! have Helix installed with grammars built.

use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::{info, warn};
use tree_house::tree_sitter::Grammar;

/// Errors that can occur when loading a grammar.
#[derive(Error, Debug)]
pub enum GrammarError {
	/// Grammar library not found in any search path.
	#[error("grammar not found: {0}")]
	NotFound(String),

	/// Failed to load the dynamic library.
	#[error("failed to load grammar library: {0}")]
	LoadError(String),

	/// Grammar library exists but doesn't export the expected symbol.
	#[error("grammar library missing language function: {0}")]
	MissingSymbol(String),

	/// Filesystem I/O error.
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
}

/// Loads a grammar by name from the search paths.
///
/// Searches all configured grammar directories for a matching shared library.
/// If the grammar is not found, returns `GrammarError::NotFound`.
///
/// For automatic fetching/building of missing grammars, use [`load_grammar_or_build`].
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

/// Loads a grammar by name, automatically fetching and building if necessary.
///
/// This function first tries to load the grammar from the search paths.
/// If not found and `grammars.kdl` contains a configuration for this grammar,
/// it will attempt to fetch the source and compile it.
///
/// This provides a "just works" experience where grammars are built on demand.
pub fn load_grammar_or_build(name: &str) -> Result<Grammar, GrammarError> {
	match load_grammar(name) {
		Ok(grammar) => return Ok(grammar),
		Err(GrammarError::NotFound(_)) => {
			info!(
				grammar = name,
				"Grammar not found, attempting to fetch and build"
			);
		}
		Err(e) => return Err(e),
	}

	if let Err(e) = auto_build_grammar(name) {
		warn!(grammar = name, error = %e, "Failed to auto-build grammar");
		return Err(GrammarError::NotFound(name.to_string()));
	}

	load_grammar(name)
}

/// Fetches grammar source from git and compiles it to a shared library.
fn auto_build_grammar(name: &str) -> Result<(), GrammarError> {
	use crate::build::{build_grammar, fetch_grammar, load_grammar_configs};

	let configs = load_grammar_configs()
		.map_err(|e| GrammarError::Io(std::io::Error::other(e.to_string())))?;

	let config = configs
		.into_iter()
		.find(|c| c.grammar_id == name)
		.ok_or_else(|| GrammarError::NotFound(format!("{} (no config in grammars.kdl)", name)))?;

	eprintln!("Fetching grammar: {name}");
	info!(grammar = name, "Fetching grammar source");
	fetch_grammar(&config)
		.map_err(|e| GrammarError::Io(std::io::Error::other(format!("fetch failed: {}", e))))?;

	eprintln!("Compiling grammar: {name}");
	info!(grammar = name, "Building grammar");
	build_grammar(&config)
		.map_err(|e| GrammarError::Io(std::io::Error::other(format!("build failed: {}", e))))?;

	info!(grammar = name, "Successfully built grammar");
	Ok(())
}

/// Loads a grammar from a specific library path.
fn load_grammar_from_path(path: &Path, name: &str) -> Result<Grammar, GrammarError> {
	// SAFETY: Loading a tree-sitter grammar from a dynamic library.
	unsafe {
		Grammar::new(name, path)
			.map_err(|e| GrammarError::LoadError(format!("{}: {}", path.display(), e)))
	}
}

/// Returns the platform-specific library filename for a grammar.
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
	/// Grammar loaded from a shared library file.
	Library(PathBuf),
	/// Grammar built into the binary.
	Builtin(&'static str),
}

/// Returns the primary runtime directory for Xeno: `~/.local/share/xeno/`.
pub fn runtime_dir() -> PathBuf {
	if let Ok(runtime) = std::env::var("XENO_RUNTIME") {
		return PathBuf::from(runtime);
	}

	data_local_dir()
		.map(|d| d.join("xeno"))
		.unwrap_or_else(|| PathBuf::from("."))
}

/// Returns the cache directory: `~/.cache/xeno/`.
pub fn cache_dir() -> Option<PathBuf> {
	#[cfg(unix)]
	{
		std::env::var_os("XDG_CACHE_HOME")
			.map(PathBuf::from)
			.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
			.map(|p| p.join("xeno"))
	}
	#[cfg(windows)]
	{
		std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("xeno").join("cache"))
	}
	#[cfg(not(any(unix, windows)))]
	{
		None
	}
}

/// Returns directories to search for compiled grammar libraries.
pub fn grammar_search_paths() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR")
		&& let Some(workspace) = PathBuf::from(manifest).ancestors().nth(2)
	{
		dirs.push(workspace.join("target").join("grammars"));
	}

	if let Some(cache) = cache_dir() {
		dirs.push(cache.join("grammars"));
	}

	if let Some(data) = data_local_dir() {
		dirs.push(data.join("xeno").join("grammars"));
	}

	for helix_dir in helix_runtime_dirs() {
		dirs.push(helix_dir.join("grammars"));
	}

	dirs
}

/// Returns directories to search for query files.
pub fn query_search_paths() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	if let Ok(runtime) = std::env::var("XENO_RUNTIME") {
		dirs.push(PathBuf::from(runtime).join("language").join("queries"));
	}

	if let Some(data) = data_local_dir() {
		dirs.push(data.join("xeno").join("queries"));
	}

	for helix_dir in helix_runtime_dirs() {
		dirs.push(helix_dir.join("queries"));
	}

	dirs
}

/// Returns the platform-specific local data directory.
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

/// Returns Helix runtime directories for fallback grammar/query loading.
fn helix_runtime_dirs() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	if let Ok(runtime) = std::env::var("HELIX_RUNTIME") {
		dirs.push(PathBuf::from(runtime));
	}

	#[cfg(unix)]
	if let Some(config) = std::env::var_os("XDG_CONFIG_HOME")
		.map(PathBuf::from)
		.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
	{
		let helix_runtime = config.join("helix").join("runtime");
		if helix_runtime.exists() {
			dirs.push(helix_runtime);
		}
	}

	if let Some(data) = data_local_dir() {
		let helix_runtime = data.join("helix").join("runtime");
		if helix_runtime.exists() {
			dirs.push(helix_runtime);
		}
	}

	dirs
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_grammar_search_paths_not_empty() {
		let dirs = grammar_search_paths();
		assert!(!dirs.is_empty());
	}

	#[test]
	fn test_query_search_paths_not_empty() {
		let dirs = query_search_paths();
		assert!(!dirs.is_empty());
	}

	#[test]
	fn test_grammar_library_name() {
		let name = grammar_library_name("rust");
		#[cfg(target_os = "linux")]
		assert_eq!(name, "librust.so");
		#[cfg(target_os = "macos")]
		assert_eq!(name, "librust.dylib");
		#[cfg(target_os = "windows")]
		assert_eq!(name, "rust.dll");
	}

	#[test]
	fn test_cache_dir_is_some() {
		#[cfg(unix)]
		assert!(cache_dir().is_some());
	}
}
