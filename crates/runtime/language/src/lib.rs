//! Tree-sitter syntax integration
//!
//! This crate provides Tree-sitter parsing, syntax highlighting, and structural
//! queries using the `tree-house` abstraction library.
//!
//! # Architecture
//!
//! - [`grammar`]: Dynamic grammar loading from shared libraries
//! - [`language`]: Language metadata (extensions, filenames, shebangs)
//! - [`loader`]: Language registry implementing `tree_house::LanguageLoader`
//! - [`query`]: Query types for indentation, text objects, tags
//! - [`highlight`]: Syntax highlighting via tree-sitter queries
//! - [`config`]: Language configuration parsing from KDL
//!
//! # Integration with Xeno
//!
//! Languages are loaded from `languages.kdl` at runtime via [`config::load_language_configs`].
//! Each language definition includes:
//! - Grammar name (for loading the .so file)
//! - File type associations (extensions, filenames, globs)
//! - Query files (highlights, indents, textobjects, injections, locals)

pub mod build;
pub mod config;
pub mod grammar;
pub mod highlight;
pub mod language;
pub mod loader;
pub mod lsp_config;
pub mod query;
pub mod runtime;
pub mod syntax;
mod utils;

pub use config::{LanguageConfigError, load_language_configs};
pub use grammar::{
	GrammarError, GrammarSource, cache_dir, grammar_search_paths, load_grammar,
	load_grammar_or_build, query_search_paths, runtime_dir,
};
pub use highlight::{Highlight, HighlightEvent, HighlightSpan, HighlightStyles, Highlighter};
pub use language::LanguageData;
pub use loader::{LanguageId, LanguageLoader};
pub use lsp_config::{
	LanguageLspInfo, LanguageLspMapping, LspConfigError, LspServerDef, load_language_lsp_mapping,
	load_lsp_configs,
};
pub use query::{CapturedNode, IndentQuery, RainbowQuery, TagQuery, TextObjectQuery, read_query};
pub use runtime::{RuntimeStatus, ensure_runtime, reseed_runtime};
