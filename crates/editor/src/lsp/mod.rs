pub(crate) mod coalesce;
pub(crate) mod code_action;
pub(crate) mod completion;
pub(crate) mod completion_filter;
pub(crate) mod diagnostics;
pub(crate) mod events;
pub(crate) mod menu;
pub(crate) mod prompt;
pub(crate) mod signature_help;
pub(crate) mod snippet;
pub(crate) mod sync_manager;
pub(crate) mod types;
pub(crate) mod workspace_edit;

pub(crate) use events::LspUiEvent;
pub(crate) use types::{LspMenuKind, LspMenuState};
// Re-export for consumers
pub use xeno_lsp::DiagnosticsEvent as LspDiagnosticsEvent;
// Re-export types needed by consumers
pub use xeno_lsp::LanguageServerConfig;
