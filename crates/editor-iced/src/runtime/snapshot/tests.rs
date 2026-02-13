use xeno_editor::Editor;

use super::*;

#[test]
fn ime_preedit_label_truncates_long_content() {
	assert_eq!(ime_preedit_label(None), "-");
	assert_eq!(ime_preedit_label(Some("short")), "short");
	assert_eq!(ime_preedit_label(Some("abcdefghijklmnopqrstuvwxyz")), "abcdefghijklmnopqrstuvwx...");
}

#[test]
fn build_snapshot_renders_document_lines_after_resize() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let snapshot = build_snapshot(&mut editor, None);
	assert!(!snapshot.document_lines.is_empty());
}
