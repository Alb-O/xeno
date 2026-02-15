//! Registry domain modules grouped behind feature flags.

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
