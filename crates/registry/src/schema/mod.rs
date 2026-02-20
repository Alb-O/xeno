//! Registry schema types for declarative domain specifications.
//!
//! These types are the single source of truth for serialized domain contracts used by
//! build-time NUON compilation and runtime blob loading.

pub mod meta;
#[allow(unused_imports)]
pub use meta::MetaCommonSpec;

pub mod actions;
pub mod commands;
pub mod grammars;
pub mod gutters;
pub mod hooks;
pub mod keymaps;
pub mod languages;
pub mod lsp_servers;
pub mod motions;
pub mod notifications;
pub mod options;
pub mod snippets;
pub mod statusline;
pub mod textobj;
pub mod themes;
