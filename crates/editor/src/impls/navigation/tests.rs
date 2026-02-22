use super::Location;
use crate::impls::Editor;

/// Verifies that LSP location → editor cursor resolves correctly for
/// non-ASCII content when the server uses UTF-16 encoding.
///
/// Text: `"a🙂bX\n"` — the emoji 🙂 occupies 2 UTF-16 code units.
/// Server returns `Position { line: 0, character: 4 }` (a=1, 🙂=2, b=1 → col 4).
/// Editor must land on `X` (char index 3), not `b` (char index 2).
#[cfg(feature = "lsp")]
#[tokio::test]
async fn goto_lsp_location_utf16_emoji() {
	let tmp = tempfile::tempdir().expect("temp dir");
	let path = tmp.path().join("emoji.rs");
	std::fs::write(&path, "a\u{1F642}bX\n").expect("write file");

	let mut editor = Editor::new(path.clone()).await.expect("open file");

	let uri = xeno_lsp::uri_from_path(&path).expect("uri");
	let lsp_loc = xeno_lsp::lsp_types::Location {
		uri,
		range: xeno_lsp::lsp_types::Range {
			start: xeno_lsp::lsp_types::Position { line: 0, character: 4 },
			end: xeno_lsp::lsp_types::Position { line: 0, character: 5 },
		},
	};

	editor
		.goto_lsp_location(&lsp_loc, xeno_lsp::OffsetEncoding::Utf16)
		.await
		.expect("goto lsp location");

	let cursor = editor.buffer().cursor;
	let ch = editor.buffer().with_doc(|doc| doc.content().char(cursor));
	assert_eq!(ch, 'X', "cursor should land on 'X', got char at index {cursor}");
}

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
	let existing_doc = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(existing_view)
		.expect("existing view buffer")
		.document_id();

	let focused = editor.focused_view();
	assert_ne!(focused, existing_view);

	editor.goto_location(&Location::new(&b_path, 0, 0)).await.expect("goto existing file");

	assert_eq!(editor.focused_view(), focused);
	assert_eq!(editor.buffer().document_id(), existing_doc);
	assert_eq!(editor.buffer_ids().len(), 2);
}
