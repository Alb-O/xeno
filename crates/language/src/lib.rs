//! Tree-sitter syntax integration for Tome editor.
//!
//! This crate provides Tree-sitter parsing, syntax highlighting, and structural
//! queries using the `tree-house` abstraction library. It follows Tome's
//! distributed slices pattern for grammar and query registration.
//!
//! # Architecture
//!
//! - [`grammar`]: Dynamic grammar loading from shared libraries
//! - [`config`]: Language configuration (grammar name, queries, injection regex)
//! - [`highlight`]: Syntax highlighting via tree-sitter queries
//!
//! # Integration with Tome
//!
//! Languages are registered via the `LANGUAGES` distributed slice in tome-manifest.
//! Each language definition includes:
//! - Grammar name (for loading the .so file)
//! - File type associations
//! - Query files (highlights, indents, textobjects, injections, locals)

pub mod config;
pub mod grammar;
pub mod highlight;

pub use config::{LanguageConfig, LanguageLoader};
pub use grammar::{GrammarError, GrammarSource};
pub use highlight::HighlightStyles;
