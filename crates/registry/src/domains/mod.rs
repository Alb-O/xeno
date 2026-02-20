//! Registry domain packages grouped behind feature flags.
//!
//! Each domain follows a colocated package layout:
//! * `contract/*`: schema-facing contracts and entry/input definitions
//! * `compile/*`: embedded blob loaders and spec→linked-def wiring
//! * `runtime/*`: typed runtime views and query helpers
//! * `exec/*`: handler/effects/controller surfaces for executable domains

#[cfg(feature = "minimal")]
pub mod catalog;
#[cfg(feature = "minimal")]
pub mod relations;
#[cfg(feature = "minimal")]
pub mod shared;

#[cfg(feature = "actions")]
pub mod actions;
#[cfg(feature = "commands")]
pub mod commands;
#[cfg(feature = "gutter")]
pub mod gutter;
#[cfg(feature = "hooks")]
#[macro_use]
pub mod hooks;
pub mod grammars;
#[cfg(feature = "languages")]
pub mod languages;
#[cfg(feature = "minimal")]
pub mod lsp_servers;
#[cfg(feature = "motions")]
pub mod motions;
#[cfg(feature = "notifications")]
pub mod notifications;
#[cfg(feature = "options")]
pub mod options;
#[cfg(feature = "commands")]
pub mod snippets;
#[cfg(feature = "statusline")]
pub mod statusline;
#[cfg(feature = "textobj")]
pub mod textobj;
#[cfg(feature = "themes")]
pub mod themes;
