use super::Location;
use crate::impls::Editor;

#[tokio::test]
async fn goto_location_keeps_focused_view_stable() {
	let tmp = tempfile::tempdir().expect("temp dir");
	let a_path = tmp.path().join("a.rs");
	let b_path = tmp.path().join("b.rs");
	std::fs::write(&a_path, "alpha\n").expect("write a");
	std::fs::write(&b_path, "beta\n").expect("write b");

	let mut editor = Editor::new(a_path).await.expect("open initial file");
	let focused = editor.focused_view();

	editor.goto_location(&Location::new(&b_path, 0, 0)).await.expect("goto location");

	assert_eq!(editor.focused_view(), focused);
	assert_eq!(editor.buffer().path(), Some(crate::paths::fast_abs(&b_path)));
}

#[tokio::test]
async fn goto_location_reuses_existing_document_without_new_view() {
	let tmp = tempfile::tempdir().expect("temp dir");
	let b_path = tmp.path().join("b.rs");
	std::fs::write(&b_path, "beta\n").expect("write b");

	let mut editor = Editor::new_scratch();
	let existing_view = editor.open_file(b_path.clone()).await.expect("open hidden file view");
	let existing_doc = editor.state.core.editor.buffers.get_buffer(existing_view).expect("existing view buffer").document_id();

	let focused = editor.focused_view();
	assert_ne!(focused, existing_view);

	editor.goto_location(&Location::new(&b_path, 0, 0)).await.expect("goto existing file");

	assert_eq!(editor.focused_view(), focused);
	assert_eq!(editor.buffer().document_id(), existing_doc);
	assert_eq!(editor.buffer_ids().len(), 2);
}
