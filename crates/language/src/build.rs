//! Grammar fetching and building system.
//!
//! This module handles fetching tree-sitter grammar sources from git repositories
//! and compiling them into dynamic libraries that can be loaded at runtime.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::{fs, thread};

use serde::Deserialize;
use thiserror::Error;

use crate::grammar::{cache_dir, grammar_search_paths, runtime_dir};

/// Errors that can occur during grammar fetching or building.
#[derive(Debug, Error)]
pub enum GrammarBuildError {
	#[error("git is not available on PATH")]
	GitNotAvailable,
	#[error("failed to read languages.toml: {0}")]
	ConfigRead(#[from] std::io::Error),
	#[error("failed to parse languages.toml: {0}")]
	ConfigParse(#[from] toml::de::Error),
	#[error("git command failed: {0}")]
	GitCommand(String),
	#[error("compilation failed: {0}")]
	Compilation(String),
	#[error("no parser.c found in {0}")]
	NoParserSource(PathBuf),
}

/// Result type for grammar operations.
pub type Result<T> = std::result::Result<T, GrammarBuildError>;

/// Grammar configuration from languages.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct GrammarConfig {
	/// The grammar name (used for the output library name).
	#[serde(rename = "name")]
	pub grammar_id: String,
	/// The source location for the grammar.
	pub source: GrammarSource,
}

/// Source location for a grammar.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum GrammarSource {
	/// A local path to the grammar source.
	Local { path: String },
	/// A git repository containing the grammar.
	Git {
		#[serde(rename = "git")]
		remote: String,
		#[serde(rename = "rev")]
		revision: String,
		/// Optional subdirectory within the repository.
		subpath: Option<String>,
	},
}

/// Languages configuration file structure.
#[derive(Debug, Deserialize)]
struct LanguagesConfig {
	#[serde(default)]
	grammar: Vec<GrammarConfig>,
}

/// Status of a fetch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
	/// Grammar was already up to date.
	UpToDate,
	/// Grammar was updated to a new revision.
	Updated,
	/// Grammar uses a local path (no fetch needed).
	Local,
}

/// Status of a build operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildStatus {
	/// Grammar was already built and up to date.
	AlreadyBuilt,
	/// Grammar was newly built.
	Built,
}

/// Embedded languages.toml from the runtime directory.
const LANGUAGES_TOML: &str = include_str!("../../../runtime/languages.toml");

/// Loads grammar configurations from the embedded `languages.toml`.
pub fn load_grammar_configs() -> Result<Vec<GrammarConfig>> {
	let config: LanguagesConfig = toml::from_str(LANGUAGES_TOML)?;
	Ok(config.grammar)
}

/// Check if git is available on PATH.
fn ensure_git_available() -> Result<()> {
	Command::new("git")
		.arg("--version")
		.output()
		.map_err(|_| GrammarBuildError::GitNotAvailable)?;
	Ok(())
}

/// Get the directory where grammar sources are stored.
///
/// Grammar sources are stored in the cache directory since they can be
/// re-fetched at any time.
pub fn grammar_sources_dir() -> PathBuf {
	cache_dir()
		.unwrap_or_else(|| runtime_dir())
		.join("grammars")
		.join("sources")
}

/// Get the directory where compiled grammars are stored.
pub fn grammar_lib_dir() -> PathBuf {
	// Use the first grammar search path, or fall back to runtime/grammars
	grammar_search_paths()
		.first()
		.cloned()
		.unwrap_or_else(|| runtime_dir().join("grammars"))
}

/// Fetch a single grammar from its git repository.
pub fn fetch_grammar(grammar: &GrammarConfig) -> Result<FetchStatus> {
	let GrammarSource::Git {
		remote, revision, ..
	} = &grammar.source
	else {
		return Ok(FetchStatus::Local);
	};

	ensure_git_available()?;

	let grammar_dir = grammar_sources_dir().join(&grammar.grammar_id);
	fs::create_dir_all(&grammar_dir)?;

	if grammar_dir.join(".git").exists() {
		// Repository exists, fetch and checkout the revision
		let fetch_output = Command::new("git")
			.args(["fetch", "--depth", "1", "origin", revision])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !fetch_output.status.success() {
			return Err(GrammarBuildError::GitCommand(
				String::from_utf8_lossy(&fetch_output.stderr).to_string(),
			));
		}

		// Check if we're already at the right revision
		let rev_parse = Command::new("git")
			.args(["rev-parse", "HEAD"])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		let current_rev = String::from_utf8_lossy(&rev_parse.stdout)
			.trim()
			.to_string();

		if current_rev.starts_with(revision) || revision.starts_with(&current_rev) {
			return Ok(FetchStatus::UpToDate);
		}

		// Checkout the new revision
		let checkout_output = Command::new("git")
			.args(["checkout", "FETCH_HEAD"])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !checkout_output.status.success() {
			return Err(GrammarBuildError::GitCommand(
				String::from_utf8_lossy(&checkout_output.stderr).to_string(),
			));
		}

		Ok(FetchStatus::Updated)
	} else {
		let clone_output = Command::new("git")
			.args([
				"clone",
				"--depth",
				"1",
				"--single-branch",
				remote,
				grammar_dir.to_str().unwrap(),
			])
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !clone_output.status.success() {
			return Err(GrammarBuildError::GitCommand(
				String::from_utf8_lossy(&clone_output.stderr).to_string(),
			));
		}

		// Fetch the specific revision
		let fetch_output = Command::new("git")
			.args(["fetch", "--depth", "1", "origin", revision])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !fetch_output.status.success() {
			// Try without depth for older git versions or if revision is a branch
			let fetch_output = Command::new("git")
				.args(["fetch", "origin", revision])
				.current_dir(&grammar_dir)
				.output()
				.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

			if !fetch_output.status.success() {
				return Err(GrammarBuildError::GitCommand(
					String::from_utf8_lossy(&fetch_output.stderr).to_string(),
				));
			}
		}

		// Checkout the revision
		let checkout_output = Command::new("git")
			.args(["checkout", revision])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !checkout_output.status.success() {
			// Try FETCH_HEAD if direct checkout fails
			let checkout_output = Command::new("git")
				.args(["checkout", "FETCH_HEAD"])
				.current_dir(&grammar_dir)
				.output()
				.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

			if !checkout_output.status.success() {
				return Err(GrammarBuildError::GitCommand(
					String::from_utf8_lossy(&checkout_output.stderr).to_string(),
				));
			}
		}

		Ok(FetchStatus::Updated)
	}
}

/// Get the source directory for a grammar (where parser.c lives).
fn get_grammar_src_dir(grammar: &GrammarConfig) -> PathBuf {
	match &grammar.source {
		GrammarSource::Local { path } => PathBuf::from(path).join("src"),
		GrammarSource::Git { subpath, .. } => {
			let base = grammar_sources_dir().join(&grammar.grammar_id);
			match subpath {
				Some(sub) => base.join(sub).join("src"),
				None => base.join("src"),
			}
		}
	}
}

/// Check if a grammar needs to be recompiled.
fn needs_recompile(src_dir: &Path, lib_path: &Path) -> bool {
	if !lib_path.exists() {
		return true;
	}

	let lib_mtime = match fs::metadata(lib_path).and_then(|m| m.modified()) {
		Ok(t) => t,
		Err(_) => return true,
	};

	// Check if any source file is newer than the library
	let source_files = ["parser.c", "scanner.c", "scanner.cc"];
	for file in source_files {
		let src_path = src_dir.join(file);
		if src_path.exists() {
			if let Ok(meta) = fs::metadata(&src_path) {
				if let Ok(src_mtime) = meta.modified() {
					if src_mtime > lib_mtime {
						return true;
					}
				}
			}
		}
	}

	false
}

/// Get the library file extension for the current platform.
#[cfg(target_os = "windows")]
fn library_extension() -> &'static str {
	"dll"
}

#[cfg(target_os = "macos")]
fn library_extension() -> &'static str {
	"dylib"
}

#[cfg(all(unix, not(target_os = "macos")))]
fn library_extension() -> &'static str {
	"so"
}

/// Build a single grammar into a dynamic library.
pub fn build_grammar(grammar: &GrammarConfig) -> Result<BuildStatus> {
	let src_dir = get_grammar_src_dir(grammar);
	let parser_path = src_dir.join("parser.c");

	if !parser_path.exists() {
		return Err(GrammarBuildError::NoParserSource(src_dir));
	}

	let lib_dir = grammar_lib_dir();
	fs::create_dir_all(&lib_dir)?;

	// Use lib prefix to match what load_grammar() expects
	let lib_name = format!(
		"lib{}.{}",
		grammar.grammar_id.replace('-', "_"),
		library_extension()
	);
	let lib_path = lib_dir.join(&lib_name);

	if !needs_recompile(&src_dir, &lib_path) {
		return Ok(BuildStatus::AlreadyBuilt);
	}

	// Build using cc crate
	// Set HOST and TARGET env vars if not present (needed outside cargo)
	let target = std::env::var("TARGET")
		.unwrap_or_else(|_| std::env::consts::ARCH.to_string() + "-unknown-linux-gnu");
	// SAFETY: We're setting env vars before any multi-threaded work happens in the cc crate
	unsafe {
		std::env::set_var("TARGET", &target);
		std::env::set_var("HOST", &target);
	}

	let mut build = cc::Build::new();
	build
		.opt_level(3)
		.cargo_metadata(false)
		.warnings(false)
		.include(&src_dir)
		.host(&target)
		.target(&target);

	build.file(&parser_path);

	let scanner_c = src_dir.join("scanner.c");
	let scanner_cc = src_dir.join("scanner.cc");

	if scanner_cc.exists() {
		build.cpp(true);
		build.file(&scanner_cc);
		build.std("c++14");
	} else if scanner_c.exists() {
		build.file(&scanner_c);
	}

	let obj_dir = lib_dir.join("obj").join(&grammar.grammar_id);
	fs::create_dir_all(&obj_dir)?;
	build.out_dir(&obj_dir);

	let _objects = build
		.try_compile(&grammar.grammar_id)
		.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

	compile_shared_library(&grammar.grammar_id, &src_dir, &lib_path)?;

	Ok(BuildStatus::Built)
}

/// Compile a grammar directly into a shared library using the system compiler.
fn compile_shared_library(_name: &str, src_dir: &Path, lib_path: &Path) -> Result<()> {
	let parser_path = src_dir.join("parser.c");
	let scanner_c = src_dir.join("scanner.c");
	let scanner_cc = src_dir.join("scanner.cc");

	#[cfg(unix)]
	{
		let compiler = if scanner_cc.exists() { "c++" } else { "cc" };

		let mut cmd = Command::new(compiler);
		cmd.arg("-shared")
			.arg("-fPIC")
			.arg("-O3")
			.arg("-fno-exceptions")
			.arg("-I")
			.arg(src_dir)
			.arg("-o")
			.arg(lib_path);

		cmd.arg(&parser_path);

		if scanner_cc.exists() {
			cmd.arg("-std=c++14");
			cmd.arg(&scanner_cc);
			cmd.arg("-lstdc++");
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		// Security hardening on Linux
		#[cfg(target_os = "linux")]
		{
			cmd.arg("-Wl,-z,relro,-z,now");
		}

		let output = cmd
			.output()
			.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

		if !output.status.success() {
			return Err(GrammarBuildError::Compilation(
				String::from_utf8_lossy(&output.stderr).to_string(),
			));
		}
	}

	#[cfg(windows)]
	{
		// Windows compilation using MSVC
		let mut cmd = Command::new("cl.exe");
		cmd.arg("/nologo")
			.arg("/LD")
			.arg("/O2")
			.arg("/utf-8")
			.arg(format!("/I{}", src_dir.display()))
			.arg(format!("/Fe:{}", lib_path.display()))
			.arg(&parser_path);

		if scanner_cc.exists() {
			cmd.arg("/std:c++14");
			cmd.arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		let output = cmd
			.output()
			.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

		if !output.status.success() {
			return Err(GrammarBuildError::Compilation(
				String::from_utf8_lossy(&output.stderr).to_string(),
			));
		}
	}

	Ok(())
}

/// Callback type for progress reporting.
pub type ProgressCallback = Box<dyn Fn(&str, &str) + Send + Sync>;

/// Fetch all grammars in parallel.
pub fn fetch_all_grammars(
	grammars: Vec<GrammarConfig>,
	on_progress: Option<ProgressCallback>,
) -> Vec<(GrammarConfig, Result<FetchStatus>)> {
	let (tx, rx) = mpsc::channel();
	let num_jobs = std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.min(8);

	let chunk_size = (grammars.len() / num_jobs).max(1);
	let chunks: Vec<Vec<GrammarConfig>> = grammars.chunks(chunk_size).map(|c| c.to_vec()).collect();

	for chunk in chunks {
		let tx = tx.clone();

		thread::spawn(move || {
			for grammar in chunk {
				let result = fetch_grammar(&grammar);
				let _ = tx.send((grammar, result));
			}
		});
	}

	drop(tx);

	let mut results = Vec::new();
	for (grammar, result) in rx {
		if let Some(ref cb) = on_progress {
			let status = match &result {
				Ok(FetchStatus::UpToDate) => "up to date",
				Ok(FetchStatus::Updated) => "updated",
				Ok(FetchStatus::Local) => "local",
				Err(_) => "error",
			};
			cb(&grammar.grammar_id, status);
		}
		results.push((grammar, result));
	}

	results
}

/// Build all grammars in parallel.
pub fn build_all_grammars(
	grammars: Vec<GrammarConfig>,
	on_progress: Option<ProgressCallback>,
) -> Vec<(GrammarConfig, Result<BuildStatus>)> {
	let (tx, rx) = mpsc::channel();
	let num_jobs = std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.min(8);

	let chunk_size = (grammars.len() / num_jobs).max(1);
	let chunks: Vec<Vec<GrammarConfig>> = grammars.chunks(chunk_size).map(|c| c.to_vec()).collect();

	for chunk in chunks {
		let tx = tx.clone();

		thread::spawn(move || {
			for grammar in chunk {
				let result = build_grammar(&grammar);
				let _ = tx.send((grammar, result));
			}
		});
	}

	drop(tx);

	let mut results = Vec::new();
	for (grammar, result) in rx {
		if let Some(ref cb) = on_progress {
			let status = match &result {
				Ok(BuildStatus::AlreadyBuilt) => "up to date",
				Ok(BuildStatus::Built) => "built",
				Err(_) => "error",
			};
			cb(&grammar.grammar_id, status);
		}
		results.push((grammar, result));
	}

	results
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_load_grammar_configs() {
		// This test will pass if languages.toml doesn't exist or is valid
		let result = load_grammar_configs();
		assert!(result.is_ok());
	}

	#[test]
	fn test_grammar_source_deserialization() {
		let toml_git = r#"
            [[grammar]]
            name = "rust"
            source = { git = "https://github.com/tree-sitter/tree-sitter-rust", rev = "abc123" }
        "#;

		let config: LanguagesConfig = toml::from_str(toml_git).unwrap();
		assert_eq!(config.grammar.len(), 1);
		assert_eq!(config.grammar[0].grammar_id, "rust");
		assert!(matches!(
			config.grammar[0].source,
			GrammarSource::Git { .. }
		));

		let toml_local = r#"
            [[grammar]]
            name = "custom"
            source = { path = "/path/to/grammar" }
        "#;

		let config: LanguagesConfig = toml::from_str(toml_local).unwrap();
		assert_eq!(config.grammar.len(), 1);
		assert!(matches!(
			config.grammar[0].source,
			GrammarSource::Local { .. }
		));
	}

	#[test]
	fn test_library_extension() {
		let ext = library_extension();
		#[cfg(target_os = "linux")]
		assert_eq!(ext, "so");
		#[cfg(target_os = "macos")]
		assert_eq!(ext, "dylib");
		#[cfg(target_os = "windows")]
		assert_eq!(ext, "dll");
	}
}
