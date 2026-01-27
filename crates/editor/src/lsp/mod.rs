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
pub(crate) mod events;
#[cfg(feature = "lsp")]
pub(crate) mod menu;
#[cfg(feature = "lsp")]
pub(crate) mod signature_help;
#[cfg(feature = "lsp")]
pub(crate) mod snippet;
#[cfg(feature = "lsp")]
pub(crate) mod sync_manager;
#[cfg(feature = "lsp")]
pub(crate) mod types;
#[cfg(feature = "lsp")]
pub(crate) mod workspace_edit;

pub mod system;

#[cfg(feature = "lsp")]
pub(crate) use events::LspUiEvent;
pub use system::LspSystem;
#[cfg(feature = "lsp")]
pub(crate) use types::{LspMenuKind, LspMenuState};
// Re-export for consumers
#[cfg(feature = "lsp")]
pub use xeno_lsp::DiagnosticsEvent as LspDiagnosticsEvent;
// Re-export types needed by consumers
#[cfg(feature = "lsp")]
pub use xeno_lsp::LanguageServerConfig;
