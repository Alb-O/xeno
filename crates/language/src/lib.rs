#![deny(clippy::print_stderr)]

//! Tree-sitter syntax integration
//!
//! This crate provides Tree-sitter parsing, syntax highlighting, and structural
//! queries using the `xeno-tree-house` abstraction library.
//! Grammar operations in this crate must emit diagnostics through tracing, not
//! stderr.
//!
//! # Architecture
//!
//! * [`grammar`]: Dynamic grammar loading from shared libraries
//! * [`language`]: Language metadata (extensions, filenames, shebangs)
//! * [`loader`]: Language registry implementing `xeno_tree_house::LanguageLoader`
//! * [`query`]: Query types for indentation, text objects, tags
//! * [`highlight`]: Syntax highlighting via tree-sitter queries
//! * [`build`]: Grammar source configuration and grammar build orchestration
//! * [`lsp_config`]: Language-to-LSP server mapping configuration
//!
//! # Integration with Xeno
//!
//! Runtime language entries are loaded through [`db::language_db`], and grammar
//! source/build metadata is loaded through [`build::load_grammar_configs`].
//! Each language definition includes:
//! * Grammar name (for loading the .so file)
//! * File type associations (extensions, filenames, globs)
//! * Query files (highlights, indents, textobjects, injections, locals)

mod build;
mod db;
mod grammar;
mod highlight;
mod ids;
mod language;
mod loader;
mod lsp_config;
mod query;
mod runtime;
mod syntax;

pub use build::{BuildStatus, FetchStatus, GrammarBuildError, GrammarConfig, build_all_grammars, build_grammar, fetch_all_grammars, fetch_grammar, load_grammar_configs};
pub use db::{LanguageDb, language_db};
pub use grammar::{GrammarError, GrammarSource, cache_dir, grammar_search_paths, load_grammar, load_grammar_or_build, query_search_paths, runtime_dir};
pub use highlight::{Highlight, HighlightEvent, HighlightSpan, HighlightStyles, Highlighter};
pub use ids::{RegistryLanguageIdExt, TreeHouseLanguageExt};
pub use language::LanguageData;
pub use loader::{LanguageId, LanguageLoader};
pub use lsp_config::{
	LanguageLspInfo, LanguageLspMapping, LspConfigError, LspServerDef, ResolvedLanguageLspConfig, load_lsp_configs, load_resolved_lsp_configs,
};
pub use query::{CapturedNode, IndentQuery, RainbowQuery, TagQuery, TextObjectQuery, read_query};
pub use syntax::{InjectionPolicy, SealedSource, Syntax, SyntaxError, SyntaxOptions, ViewportRepair};
