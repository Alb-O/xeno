use super::Editor;

#[test]
fn editor_starts_with_scratch_buffer() {
	let editor = Editor::new_scratch();
	let bytes = editor.buffer().with_doc(|doc| doc.content().len_bytes());
	assert_eq!(bytes, 0);
}
