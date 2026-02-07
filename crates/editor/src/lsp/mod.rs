#[cfg(feature = "lsp")]
pub(crate) mod coalesce;
#[cfg(feature = "lsp")]
pub(crate) mod code_action;
#[cfg(feature = "lsp")]
pub(crate) mod completion;
#[cfg(feature = "lsp")]
pub(crate) mod completion_filter;
#[cfg(feature = "lsp")]
pub(crate) mod diagnostics;
#[cfg(feature = "lsp")]
mod document_ops;
#[cfg(feature = "lsp")]
mod encoding;
#[cfg(feature = "lsp")]
pub(crate) mod events;
#[cfg(feature = "lsp")]
pub(crate) mod menu;
mod render;
#[cfg(feature = "lsp")]
mod requests;
#[cfg(feature = "lsp")]
pub(crate) mod signature_help;
#[cfg(feature = "lsp")]
pub mod smoke;
#[cfg(feature = "lsp")]
pub(crate) mod snippet;
#[cfg(feature = "lsp")]
pub(crate) mod sync_manager;
#[cfg(feature = "lsp")]
pub(crate) mod types;
#[cfg(feature = "lsp")]
pub(crate) mod workspace_edit;

pub mod api;
pub mod system;

#[cfg(feature = "lsp")]
pub(crate) use events::LspUiEvent;
#[cfg(feature = "lsp")]
pub use system::LspHandle;
pub(crate) use system::LspSystem;
#[cfg(feature = "lsp")]
pub(crate) use types::{LspMenuKind, LspMenuState};
// Re-export for consumers
#[cfg(feature = "lsp")]
pub use xeno_lsp::DiagnosticsEvent as LspDiagnosticsEvent;
