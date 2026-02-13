use super::{collect_iced_digest, collect_tui_digest};
use crate::Editor;
use crate::info_popup::PopupAnchor;

fn make_editor(cols: u16, rows: u16) -> Editor {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(cols, rows);
	editor.state.viewport.doc_area = Some(editor.doc_area());
	editor
}

fn assert_convergence(editor: &mut Editor) {
	let bounds = editor.doc_area();
	let tui = collect_tui_digest(editor, bounds);
	let iced = collect_iced_digest(editor, bounds);
	assert_eq!(tui, iced, "TUI and Iced digests diverge");
}

#[test]
fn baseline_after_resize() {
	let mut editor = make_editor(80, 24);
	assert_convergence(&mut editor);

	let bounds = editor.doc_area();
	let digest = collect_tui_digest(&mut editor, bounds);
	assert!(digest.panes.is_empty());
	assert!(digest.popups.is_empty());
	assert!(digest.completion.is_none());
	assert!(digest.status.rows > 0);
}

#[test]
fn info_popup_center_converges() {
	let mut editor = make_editor(80, 24);
	editor.open_info_popup("Hello popup".to_string(), None, PopupAnchor::Center);
	assert_convergence(&mut editor);

	let bounds = editor.doc_area();
	let digest = collect_tui_digest(&mut editor, bounds);
	assert_eq!(digest.popups.len(), 1);
}

#[test]
fn info_popup_point_converges() {
	let mut editor = make_editor(80, 24);
	editor.open_info_popup("Point popup".to_string(), None, PopupAnchor::Point { x: 10, y: 5 });
	assert_convergence(&mut editor);
}

#[test]
fn info_popup_close_returns_to_baseline() {
	let mut editor = make_editor(80, 24);

	let bounds = editor.doc_area();
	let baseline = collect_tui_digest(&mut editor, bounds);

	let popup_id = editor.open_info_popup("Temporary".to_string(), None, PopupAnchor::Center).unwrap();
	let bounds = editor.doc_area();
	let with_popup = collect_tui_digest(&mut editor, bounds);
	assert_ne!(baseline, with_popup);

	editor.close_info_popup(popup_id);
	assert_convergence(&mut editor);

	let bounds = editor.doc_area();
	let after_close = collect_tui_digest(&mut editor, bounds);
	assert_eq!(baseline.popups, after_close.popups);
}

#[test]
fn multiple_popups_converge() {
	let mut editor = make_editor(80, 24);
	editor.open_info_popup("First".to_string(), None, PopupAnchor::Center);
	editor.open_info_popup("Second".to_string(), None, PopupAnchor::Point { x: 5, y: 3 });
	assert_convergence(&mut editor);

	let bounds = editor.doc_area();
	let digest = collect_tui_digest(&mut editor, bounds);
	assert_eq!(digest.popups.len(), 2);
}

#[test]
fn command_palette_overlay_converges() {
	let mut editor = make_editor(80, 24);
	editor.open_command_palette();
	assert_convergence(&mut editor);

	let bounds = editor.doc_area();
	let digest = collect_tui_digest(&mut editor, bounds);
	assert!(!digest.panes.is_empty(), "command palette should produce overlay panes");
}

#[test]
fn info_popup_window_anchor_converges() {
	let mut editor = make_editor(80, 24);
	let wid = editor.state.windows.base_id();
	editor.open_info_popup("Window popup".to_string(), None, PopupAnchor::Window(wid));
	assert_convergence(&mut editor);

	let bounds = editor.doc_area();
	let digest = collect_tui_digest(&mut editor, bounds);
	assert_eq!(digest.popups.len(), 1);

	let popup = &digest.popups[0];
	assert!(popup.rect.x + popup.rect.width <= bounds.x + bounds.width);
	assert!(popup.rect.y + popup.rect.height <= bounds.y + bounds.height);
}
