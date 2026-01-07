//! Built-in command implementations.

/// Buffer navigation and management commands.
mod buffer;
/// File editing commands (edit, open).
mod edit;
/// Help and documentation commands.
mod help;
/// LSP commands (hover, goto-definition).
mod lsp;
/// Quit and exit commands.
mod quit;
/// Registry diagnostic commands.
mod registry_diag;
/// Option setting commands.
mod set;
/// Test notification commands.
mod test_notify;
/// Test info popup commands.
mod test_popup;
/// Theme switching commands.
mod theme;
/// Write and save commands.
mod write;
