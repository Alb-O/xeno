//! Built-in command implementations.
//!
//! LSP commands (hover, goto-definition) are in `xeno-api::commands::lsp`
//! since they need direct [`Editor`] access.

/// Buffer navigation and management commands.
pub(super) mod buffer;
/// File editing commands (edit, open).
pub(super) mod edit;
/// Help and documentation commands.
pub(super) mod help;
/// Quit and exit commands.
pub(super) mod quit;
/// Registry diagnostic commands.
pub(super) mod registry_diag;
/// Option setting commands.
pub(super) mod set;
/// Theme switching commands.
pub(super) mod theme;
/// Write and save commands.
pub(super) mod write;
