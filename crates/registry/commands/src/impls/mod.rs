//! Built-in command implementations.
//!
//! LSP commands (hover, goto-definition) are in `xeno-api::commands::lsp`
//! since they need direct [`Editor`] access.

/// Buffer navigation and management commands.
mod buffer;
/// File editing commands (edit, open).
mod edit;
/// Help and documentation commands.
mod help;
/// Quit and exit commands.
mod quit;
/// Registry diagnostic commands.
mod registry_diag;
/// Option setting commands.
mod set;
/// Theme switching commands.
mod theme;
/// Write and save commands.
mod write;
