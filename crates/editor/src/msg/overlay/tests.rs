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
	assert_eq!(editor.pending_runtime_workspace_edit_work(), 0);

	OverlayMsg::ApplyWorkspaceEdit(empty_edit()).apply(&mut editor);

	assert_eq!(editor.pending_runtime_workspace_edit_work(), 1);
}

#[cfg(feature = "lsp")]
#[test]
fn drain_messages_processes_overlaymsg() {
	let mut editor = Editor::new_scratch();
	let tx = editor.msg_tx();
	assert!(tx.send(EditorMsg::Overlay(OverlayMsg::ApplyWorkspaceEdit(empty_edit()))).is_ok());

	let dirty = editor.drain_messages();

	assert!(dirty.needs_redraw());
	assert_eq!(editor.pending_runtime_workspace_edit_work(), 1);
}

#[cfg(feature = "lsp")]
#[tokio::test]
async fn pump_drains_deferred_workspace_edits_queue() {
	let mut editor = Editor::new_scratch();
	editor.enqueue_runtime_workspace_edit_work(empty_edit());
	assert_eq!(editor.pending_runtime_workspace_edit_work(), 1);

	let _ = editor.pump().await;

	assert_eq!(editor.pending_runtime_workspace_edit_work(), 0);
}
