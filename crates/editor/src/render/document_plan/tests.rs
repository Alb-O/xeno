use super::*;

#[test]
fn focused_document_render_plan_renders_lines_after_resize() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let plan = editor.focused_document_render_plan();
	assert!(!plan.lines.is_empty());
}

#[test]
fn focused_document_render_plan_uses_scratch_title_without_path() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.title, "[scratch]");
}

#[test]
fn focused_document_render_plan_uses_path_title_for_file_buffers() {
	let file = tempfile::NamedTempFile::new().expect("temp file");
	std::fs::write(file.path(), "alpha\n").expect("write file");

	let mut editor = Editor::new_scratch();
	let loader = editor.config().language_loader.clone();
	let _ = editor.buffer_mut().set_path(Some(file.path().to_path_buf()), Some(&loader));
	editor.handle_window_resize(80, 24);

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.title, file.path().display().to_string());
}

#[test]
fn focused_document_render_plan_returns_placeholder_for_tiny_viewport() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(1, 1);

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.lines.len(), 1);
	assert_eq!(plan.lines[0].spans[0].content.as_ref(), "document viewport too small");
}
