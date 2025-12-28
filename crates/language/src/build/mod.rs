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
pub use config::{
	GrammarConfig, GrammarSource, get_grammar_src_dir, grammar_lib_dir, grammar_sources_dir,
	library_extension, load_grammar_configs,
};
pub use fetch::{FetchStatus, fetch_grammar};
pub use parallel::{ProgressCallback, build_all_grammars, fetch_all_grammars};
use thiserror::Error;

/// Errors that can occur during grammar fetching or building.
#[derive(Debug, Error)]
pub enum GrammarBuildError {
	#[error("git is not available on PATH")]
	GitNotAvailable,
	#[error("failed to read languages.kdl: {0}")]
	ConfigRead(#[from] std::io::Error),
	#[error("failed to parse languages.kdl: {0}")]
	ConfigParseKdl(#[from] kdl::KdlError),
	#[error("invalid languages.kdl configuration: {0}")]
	ConfigParse(String),
	#[error("git command failed: {0}")]
	GitCommand(String),
	#[error("compilation failed: {0}")]
	Compilation(String),
	#[error("no parser.c found in {0}")]
	NoParserSource(PathBuf),
}

/// Result type for grammar operations.
pub type Result<T> = std::result::Result<T, GrammarBuildError>;
