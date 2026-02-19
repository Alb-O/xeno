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

	let _ = editor.drain_until_idle(crate::runtime::DrainPolicy::for_pump()).await;

	assert_eq!(editor.pending_runtime_workspace_edit_work(), 0);
}

#[cfg(feature = "lsp")]
#[test]
fn rename_stale_token_is_ignored() {
	use crate::msg::Dirty;

	let mut editor = Editor::new_scratch();
	// Simulate: first rename submitted with token=1, then a second with token=2.
	editor.state.pending_rename_token = Some(2);

	// Stale result (token=1) arrives first — should be ignored.
	let stale = OverlayMsg::RenameDone {
		token: 1,
		result: Ok(Some(empty_edit())),
	};
	let dirty = stale.apply(&mut editor);

	assert_eq!(dirty, Dirty::NONE, "stale rename token should produce Dirty::NONE");
	assert_eq!(
		editor.pending_runtime_workspace_edit_work(),
		0,
		"stale rename should not enqueue workspace edit"
	);
	assert_eq!(editor.state.pending_rename_token, Some(2), "pending token should remain");
}

#[cfg(feature = "lsp")]
#[test]
fn rename_result_ignored_after_overlay_close() {
	use crate::msg::Dirty;

	let mut editor = Editor::new_scratch();
	// Simulate: rename submitted with token=1, then overlay closed (token cleared).
	editor.state.pending_rename_token = None;

	let result = OverlayMsg::RenameDone {
		token: 1,
		result: Ok(Some(empty_edit())),
	};
	let dirty = result.apply(&mut editor);

	assert_eq!(dirty, Dirty::NONE, "rename after overlay close should be ignored");
	assert_eq!(editor.pending_runtime_workspace_edit_work(), 0, "no workspace edit should be enqueued");
}

#[cfg(feature = "lsp")]
#[test]
fn rename_current_token_applies_workspace_edit() {
	use crate::msg::Dirty;

	let mut editor = Editor::new_scratch();
	editor.state.pending_rename_token = Some(3);

	let result = OverlayMsg::RenameDone {
		token: 3,
		result: Ok(Some(empty_edit())),
	};
	let dirty = result.apply(&mut editor);

	assert_eq!(dirty, Dirty::REDRAW, "current rename token should produce Dirty::REDRAW");
	assert_eq!(editor.pending_runtime_workspace_edit_work(), 1, "workspace edit should be enqueued");
	assert_eq!(editor.state.pending_rename_token, None, "pending token should be cleared");
}
