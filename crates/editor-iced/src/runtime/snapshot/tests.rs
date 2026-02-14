use xeno_editor::Editor;

use super::*;

#[test]
fn ime_preedit_label_truncates_long_content() {
	assert_eq!(ime_preedit_label(None), "-");
	assert_eq!(ime_preedit_label(Some("short")), "short");
	assert_eq!(ime_preedit_label(Some("abcdefghijklmnopqrstuvwxyz")), "abcdefghijklmnopqrstuvwx...");
}

#[test]
fn build_snapshot_renders_document_views_after_resize() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let doc_area = editor.doc_area();

	let snapshot = build_snapshot(&mut editor, None, Some(doc_area));
	assert!(!snapshot.document_views.is_empty(), "should have at least one document view");
	assert!(!snapshot.document_views[0].text().is_empty(), "document view should have rendered text");
}

#[test]
fn build_snapshot_without_bounds_has_no_document_views() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let snapshot = build_snapshot(&mut editor, None, None);
	assert!(snapshot.document_views.is_empty());
	assert!(snapshot.separators.is_empty());
}
