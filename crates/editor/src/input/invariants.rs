use xeno_primitives::{Key, KeyCode, Modifiers, MouseButton, MouseEvent};

use crate::Editor;
use crate::impls::FocusTarget;

/// Must allow active overlay interaction to consume Enter and defer commit.
///
/// * Enforced in: `Editor::handle_key_active`
/// * Failure symptom: modal commit executes re-entrantly in key handling.
#[tokio::test]
async fn test_overlay_enter_sets_pending_commit() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_key(Key::new(KeyCode::Enter)).await;
	assert!(editor.frame().pending_overlay_commit);
	assert!(editor.overlay_kind().is_some());
}

/// Must keep overlay focus when keys are handled by active modal interaction.
///
/// * Enforced in: `Editor::handle_key_active`, overlay controller key handlers
/// * Failure symptom: typed modal input leaks into base editor state.
#[tokio::test]
async fn test_modal_key_keeps_overlay_focus() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_key(Key::char('s')).await;

	assert!(editor.overlay_kind().is_some());
	assert!(matches!(editor.focus(), FocusTarget::Overlay { .. }));
}

/// Must dismiss modal overlays on outside click.
///
/// * Enforced in: `Editor::handle_mouse_in_doc_area`
/// * Failure symptom: overlays trap focus and cannot be dismissed by pointer.
#[tokio::test]
async fn test_click_outside_modal_closes_overlay() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor
		.handle_mouse(MouseEvent::Press {
			button: MouseButton::Left,
			row: 0,
			col: 0,
			modifiers: Modifiers::NONE,
		})
		.await;

	assert!(editor.overlay_kind().is_none());
}
