//! Tree-sitter syntax integration for Tome editor.
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
//!
//! # Integration with Tome
//!
//! Languages are registered via the `LANGUAGES` distributed slice in tome-manifest.
//! Each language definition includes:
//! - Grammar name (for loading the .so file)
//! - File type associations
//! - Query files (highlights, indents, textobjects, injections, locals)

pub mod grammar;
pub mod highlight;
pub mod language;
pub mod loader;
pub mod query;
pub mod syntax;

pub use grammar::{GrammarError, GrammarSource};
pub use highlight::{Highlight, HighlightEvent, HighlightSpan, HighlightStyles, Highlighter};
pub use language::LanguageData;
pub use loader::{LanguageId, LanguageLoader};
pub use query::{read_query, CapturedNode, IndentQuery, RainbowQuery, TagQuery, TextObjectQuery};
