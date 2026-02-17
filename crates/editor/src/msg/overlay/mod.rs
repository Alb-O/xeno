//! Overlay-related async messages.

use xeno_registry::notifications::Notification;

use super::Dirty;
use crate::Editor;

/// Messages for async overlay outcomes.
#[derive(Debug)]
pub enum OverlayMsg {
	/// Emit a user notification.
	Notify(Notification),
	/// Queue a workspace edit to be applied in runtime pump.
	#[cfg(feature = "lsp")]
	ApplyWorkspaceEdit(xeno_lsp::lsp_types::WorkspaceEdit),
}

impl OverlayMsg {
	/// Applies this message to editor state, returning dirty flags.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::Notify(notification) => {
				editor.notify(notification);
				Dirty::REDRAW
			}
			#[cfg(feature = "lsp")]
			Self::ApplyWorkspaceEdit(edit) => {
				editor.enqueue_workspace_edit_deferred(edit);
				Dirty::REDRAW
			}
		}
	}
}

#[cfg(test)]
mod tests;
