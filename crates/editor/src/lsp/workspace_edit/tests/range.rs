use super::*;

#[test]
fn convert_text_edit_oob_line_returns_none() {
	let rope = xeno_primitives::Rope::from("hello\nworld\n");
	let edit = TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 99, character: 0 },
			end: lsp_types::Position { line: 99, character: 5 },
		},
		new_text: "X".into(),
	};
	assert!(convert_text_edit(&rope, OffsetEncoding::Utf16, &edit).is_none());
}

#[test]
fn convert_text_edit_reversed_range_returns_none() {
	let rope = xeno_primitives::Rope::from("hello\nworld\n");
	// end position (line 0) is before start position (line 1)
	let edit = TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 1, character: 0 },
			end: lsp_types::Position { line: 0, character: 0 },
		},
		new_text: "X".into(),
	};
	assert!(
		convert_text_edit(&rope, OffsetEncoding::Utf16, &edit).is_none(),
		"reversed range should be rejected"
	);
}

#[test]
fn convert_text_edit_oob_character_clamps() {
	let rope = xeno_primitives::Rope::from("hi\n");
	// character 999 on a 2-char line → clamped to line end
	let edit = TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 0, character: 0 },
			end: lsp_types::Position { line: 0, character: 999 },
		},
		new_text: "X".into(),
	};
	let planned = convert_text_edit(&rope, OffsetEncoding::Utf16, &edit).unwrap();
	assert_eq!(planned.range.start, 0);
	assert_eq!(planned.range.end, 2, "character should be clamped to line length");
}

#[tokio::test]
async fn workspace_edit_invalid_range_rejected_no_panic() {
	let mut editor = crate::Editor::new_scratch();
	let (path, uri, view_id) = open_temp_doc(&mut editor, "range_invalid.rs", "hello\nworld\n", 0).await;

	// OOB line in the edit range.
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range {
					start: lsp_types::Position { line: 99, character: 0 },
					end: lsp_types::Position { line: 99, character: 5 },
				},
				new_text: "NOPE".into(),
			})],
		}])),
		change_annotations: None,
	};
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(
		matches!(err.error, ApplyError::RangeConversionFailed(_)),
		"expected RangeConversionFailed, got: {err:?}"
	);

	// Original view_id must still be valid and content unchanged.
	assert_eq!(buffer_text(&editor, view_id), "hello\nworld\n", "buffer must be unchanged");

	let _ = std::fs::remove_file(path);
}
