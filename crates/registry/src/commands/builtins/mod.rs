//! Built-in command implementations.
//!
//! LSP commands (hover, goto-definition) are in `xeno-api::commands::lsp`
//! since they need direct [`Editor`] access.

/// Buffer navigation and management commands.
pub mod buffer;
/// File editing commands (edit, open).
pub mod edit;
/// Help and documentation commands.
pub mod help;
/// Quit and exit commands.
pub mod quit;
/// Registry diagnostic commands.
pub mod registry_diag;
/// Option setting commands.
pub mod set;
/// Theme switching commands.
pub mod theme;
/// Write and save commands.
pub mod write;
