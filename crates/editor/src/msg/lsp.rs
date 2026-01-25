//! LSP-related messages.

use super::Dirty;
use crate::Editor;

/// Messages for LSP lifecycle events.
#[derive(Debug)]
pub enum LspMsg {
	/// LSP catalog is ready for use.
	CatalogReady,
	/// A server failed to start.
	ServerFailed { language: String, error: String },
}

impl LspMsg {
	/// Applies this message to editor state, returning dirty flags.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::CatalogReady => {
				tracing::debug!("LSP catalog ready, initializing for open buffers");
				editor.kick_lsp_init_for_open_buffers();
				Dirty::NONE
			}
			Self::ServerFailed { language, error } => {
				tracing::warn!(language, error, "LSP server failed");
				Dirty::NONE
			}
		}
	}
}
