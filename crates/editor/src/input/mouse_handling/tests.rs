use xeno_primitives::{Modifiers, MouseButton, MouseEvent};

use crate::impls::{Editor, FocusTarget};

fn mouse_down(column: u16, row: u16) -> MouseEvent {
	MouseEvent::Press {
		button: MouseButton::Left,
		row,
		col: column,
		modifiers: Modifiers::NONE,
	}
}

#[tokio::test]
async fn modal_mouse_capture_keeps_overlay_open() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let pane = editor
		.state
		.ui.overlay_system
		.interaction()
		.active()
		.and_then(|active| active.session.panes.first())
		.expect("overlay pane should exist");

	let mouse = mouse_down(pane.rect.x, pane.rect.y);
	let _ = editor.handle_mouse(mouse).await;

	assert!(editor.state.ui.overlay_system.interaction().is_open());
	assert!(matches!(editor.focus(), FocusTarget::Overlay { .. }));
}

#[tokio::test]
async fn click_outside_modal_closes_overlay() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let mouse = mouse_down(0, 0);
	let _ = editor.handle_mouse(mouse).await;

	assert!(!editor.state.ui.overlay_system.interaction().is_open());
}
