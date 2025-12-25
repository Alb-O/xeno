//! Built-in language registrations for tree-sitter integration.
//!
//! Each language defines its grammar, file associations, and comment syntax.
//! The linkme distributed slice collects them at link time.

mod c;
mod go;
mod javascript;
mod python;
mod rust;
mod shell;
mod typescript;
