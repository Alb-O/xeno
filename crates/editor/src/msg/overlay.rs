//! Overlay-related async messages.

use xeno_registry::notifications::Notification;

use super::Dirty;
use crate::Editor;

/// Messages for async overlay outcomes.
#[derive(Debug)]
pub enum OverlayMsg {
	/// Emit a user notification.
	Notify(Notification),
	/// Request a redraw.
	RequestRedraw,
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
				Dirty::FULL
			}
			Self::RequestRedraw => {
				editor.frame_mut().needs_redraw = true;
				Dirty::FULL
			}
			#[cfg(feature = "lsp")]
			Self::ApplyWorkspaceEdit(edit) => {
				editor.frame_mut().pending_workspace_edits.push(edit);
				Dirty::FULL
			}
		}
	}
}

#[cfg(test)]
mod tests {
	#[cfg(feature = "lsp")]
	use xeno_lsp::lsp_types::WorkspaceEdit;

	#[cfg(feature = "lsp")]
	use super::OverlayMsg;
	#[cfg(feature = "lsp")]
	use crate::Editor;
	#[cfg(feature = "lsp")]
	use crate::msg::EditorMsg;

	#[cfg(feature = "lsp")]
	fn empty_edit() -> WorkspaceEdit {
		WorkspaceEdit::default()
	}

	#[cfg(feature = "lsp")]
	#[test]
	fn overlaymsg_apply_enqueues_workspace_edit() {
		let mut editor = Editor::new_scratch();
		assert!(editor.frame().pending_workspace_edits.is_empty());

		OverlayMsg::ApplyWorkspaceEdit(empty_edit()).apply(&mut editor);

		assert_eq!(editor.frame().pending_workspace_edits.len(), 1);
	}

	#[cfg(feature = "lsp")]
	#[test]
	fn drain_messages_processes_overlaymsg() {
		let mut editor = Editor::new_scratch();
		let tx = editor.msg_tx();
		assert!(
			tx.send(EditorMsg::Overlay(OverlayMsg::ApplyWorkspaceEdit(
				empty_edit()
			)))
			.is_ok()
		);

		let dirty = editor.drain_messages();

		assert!(dirty.needs_redraw());
		assert_eq!(editor.frame().pending_workspace_edits.len(), 1);
	}

	#[cfg(feature = "lsp")]
	#[tokio::test]
	async fn pump_drains_pending_workspace_edits_queue() {
		let mut editor = Editor::new_scratch();
		editor
			.frame_mut()
			.pending_workspace_edits
			.push(empty_edit());
		assert_eq!(editor.frame().pending_workspace_edits.len(), 1);

		let _ = editor.pump().await;

		assert!(editor.frame().pending_workspace_edits.is_empty());
	}
}
