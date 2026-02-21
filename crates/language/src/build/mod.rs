//! Grammar fetching and building system.
//!
//! This module handles fetching tree-sitter grammar sources from git repositories
//! and compiling them into dynamic libraries that can be loaded at runtime.

mod compile;
mod config;
mod fetch;
mod parallel;

use std::path::PathBuf;

pub use compile::{BuildStatus, build_grammar};
pub use config::{GrammarConfig, load_grammar_configs};
pub use fetch::{FetchStatus, fetch_grammar};
pub use parallel::{build_all_grammars, fetch_all_grammars};
use thiserror::Error;

/// Errors that can occur during grammar fetching or building.
#[derive(Debug, Error)]
pub enum GrammarBuildError {
	/// Git executable not found in PATH.
	#[error("git is not available on PATH")]
	GitNotAvailable,
	/// Failed to read the languages configuration file.
	#[error("failed to read languages config: {0}")]
	ConfigRead(#[from] std::io::Error),
	/// Semantic error in languages configuration.
	#[error("invalid languages configuration: {0}")]
	ConfigParse(String),
	/// Git clone, fetch, or checkout failed.
	#[error("git command failed: {0}")]
	GitCommand(String),
	/// C/C++ compilation of the grammar failed.
	#[error("compilation failed: {0}")]
	Compilation(String),
	/// The grammar source directory lacks a parser.c file.
	#[error("no parser.c found in {0}")]
	NoParserSource(PathBuf),
}

/// Result type for grammar operations.
pub type Result<T> = std::result::Result<T, GrammarBuildError>;
