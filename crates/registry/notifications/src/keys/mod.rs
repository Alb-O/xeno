//! Notification keys organized by domain.
//!
//! Keys are split into separate files by domain to support future expansion.
//! As more features are added, each domain can grow independently without
//! creating a monolithic file.
//!
//! # Adding New Keys
//!
//! Add keys to the appropriate domain module. If a notification doesn't fit
//! existing domains, consider whether it belongs in `builtins` (generic) or
//! warrants a new domain module.

mod actions;
mod builtins;
mod commands;
mod editor;
mod runtime;

pub use actions::keys::*;
pub use builtins::keys::*;
pub use commands::keys::*;
pub use editor::keys::*;
pub use runtime::keys::*;
