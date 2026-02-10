use super::Editor;

#[test]
fn ensure_syntax_for_buffers_clamps_stale_scroll_line_after_large_delete() {
	let mut editor = Editor::new_scratch();

	let mut large = String::new();
	for _ in 0..100_000 {
		large.push_str("line\n");
	}

	{
		let buffer = editor.buffer_mut();
		buffer.reset_content(large);
		buffer.scroll_line = 95_800;
		buffer.reset_content("collapsed\n");
	}

	editor.ensure_syntax_for_buffers();
}
