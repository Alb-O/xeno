//! Links KDL metadata with Rust handler functions for actions and commands.
//!
//! At startup, `link_actions` and `link_commands` pair each `*MetaRaw` from
//! precompiled blobs with handler statics collected via `inventory`. The result
//! is `Linked*Def` types that implement `BuildEntry` for the registry builder.

use crate::core::FrozenInterner;

#[cfg(feature = "actions")]
pub mod actions;
pub(crate) mod common;
pub(crate) mod parse;
pub(crate) mod spec;
#[cfg(feature = "actions")]
pub use actions::*;

#[cfg(feature = "commands")]
pub mod commands;
#[cfg(feature = "commands")]
pub use commands::*;

#[cfg(feature = "motions")]
pub mod motions;
#[cfg(feature = "motions")]
pub use motions::*;

#[cfg(feature = "textobj")]
pub mod textobj;
#[cfg(feature = "textobj")]
pub use textobj::*;

#[cfg(feature = "gutter")]
pub mod gutter;
#[cfg(feature = "gutter")]
pub use gutter::*;

#[cfg(feature = "statusline")]
pub mod statusline;
#[cfg(feature = "statusline")]
pub use statusline::*;

#[cfg(feature = "hooks")]
pub mod hooks;
#[cfg(feature = "hooks")]
pub use hooks::*;

#[cfg(feature = "options")]
pub mod options;
#[cfg(feature = "options")]
pub use options::*;

#[cfg(feature = "notifications")]
pub mod notifications;
#[cfg(feature = "notifications")]
pub use notifications::*;

#[cfg(feature = "themes")]
pub mod themes;
#[cfg(feature = "themes")]
pub use themes::*;

#[cfg(test)]
mod tests;
