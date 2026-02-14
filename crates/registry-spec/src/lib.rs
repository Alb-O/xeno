#[cfg(feature = "compile")]
pub mod compile;
#[cfg(feature = "compile")]
pub(crate) mod nu_de;

pub mod meta;
pub use meta::MetaCommonSpec;

pub mod actions;
pub mod commands;
pub mod grammars;
pub mod gutters;
pub mod hooks;
pub mod languages;
pub mod lsp_servers;
pub mod motions;
pub mod notifications;
pub mod options;
pub mod snippets;
pub mod statusline;
pub mod textobj;
pub mod themes;
