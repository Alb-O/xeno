use std::path::Path;

use super::*;

#[tokio::test]
async fn workspace_edit_error_does_not_reopen_existing_buffer() {
	let mut editor = crate::Editor::new_scratch();
	let (path, _uri, view_id) = open_temp_doc(&mut editor, "identity_error.rs", "original\n", 3).await;
	let uri = register_doc_at_version(&editor, &path, 3);

	// Version mismatch → error, but the buffer should not be closed/reopened.
	let edit = versioned_workspace_edit(uri, Some(0));
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(matches!(err.error, ApplyError::VersionMismatch { .. }));

	// Same view_id must still be valid.
	assert_eq!(buffer_text(&editor, view_id), "original\n", "buffer identity must be preserved");
	assert_eq!(
		editor.state.core.editor.buffers.find_by_path(&path),
		Some(view_id),
		"view_id for path must be the same as before"
	);

	let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn workspace_edit_success_does_not_reopen_existing_buffer() {
	let mut editor = crate::Editor::new_scratch();
	let (path, _uri, view_id) = open_temp_doc(&mut editor, "identity_success.rs", "old\n", 0).await;

	// Unversioned edit targeting the same buffer → should succeed without
	// closing/reopening the buffer.
	let uri: Uri = xeno_lsp::uri_from_path(&path).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range {
					start: lsp_types::Position { line: 0, character: 0 },
					end: lsp_types::Position { line: 0, character: 3 },
				},
				new_text: "new".into(),
			})],
		}])),
		change_annotations: None,
	};
	editor.apply_workspace_edit(edit).await.unwrap();

	// Same view_id, text changed.
	assert_eq!(
		editor.state.core.editor.buffers.find_by_path(&path),
		Some(view_id),
		"view_id must be preserved on successful edit"
	);
	assert_eq!(buffer_text(&editor, view_id), "new\n", "text should be updated in-place");

	let _ = std::fs::remove_file(path);
}

// --- Temporary buffer lifecycle tests ---

#[tokio::test]
async fn workspace_edit_temporary_buffers_closed_on_success() {
	let mut editor = crate::Editor::new_scratch();
	let path = create_temp_file("temp_success.rs", "old\n");

	// Buffer not opened yet — workspace edit will open it temporarily.
	assert!(editor.state.core.editor.buffers.find_by_path(&path).is_none());

	let uri = xeno_lsp::uri_from_path(&path).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range {
					start: lsp_types::Position { line: 0, character: 0 },
					end: lsp_types::Position { line: 0, character: 3 },
				},
				new_text: "new".into(),
			})],
		}])),
		change_annotations: None,
	};
	editor.apply_workspace_edit(edit).await.unwrap();

	// Temporary buffer should be closed after successful apply.
	assert!(
		editor.state.core.editor.buffers.find_by_path(&path).is_none(),
		"temporary buffer should be closed after successful workspace edit"
	);

	let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn workspace_edit_temporary_buffers_closed_on_error() {
	let mut editor = crate::Editor::new_scratch();

	// Two files: temp_a.rs (valid edit, unversioned) and temp_b.rs (OOB range → error).
	let path_a = create_temp_file("temp_err_a.rs", "aaa\n");
	let path_b = create_temp_file("temp_err_b.rs", "bbb\n");

	assert!(editor.state.core.editor.buffers.find_by_path(&path_a).is_none());
	assert!(editor.state.core.editor.buffers.find_by_path(&path_b).is_none());

	let uri_a = xeno_lsp::uri_from_path(&path_a).unwrap();
	let uri_b = xeno_lsp::uri_from_path(&path_b).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_a, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range::default(),
					new_text: "AAA".into(),
				})],
			},
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_b, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range {
						start: lsp_types::Position { line: 99, character: 0 },
						end: lsp_types::Position { line: 99, character: 5 },
					},
					new_text: "BBB".into(),
				})],
			},
		])),
		change_annotations: None,
	};
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(matches!(err.error, ApplyError::RangeConversionFailed(_)));

	// Both temp buffers should be cleaned up despite the error.
	assert!(
		editor.state.core.editor.buffers.find_by_path(&path_a).is_none(),
		"temp buffer A should be closed after error"
	);
	assert!(
		editor.state.core.editor.buffers.find_by_path(&path_b).is_none(),
		"temp buffer B should be closed after error"
	);

	let _ = std::fs::remove_file(path_a);
	let _ = std::fs::remove_file(path_b);
}

#[tokio::test]
async fn workspace_edit_does_not_close_preexisting_buffers() {
	let mut editor = crate::Editor::new_scratch();
	let (path, _uri, view_id) = open_temp_doc(&mut editor, "preexisting.rs", "keep_me\n", 0).await;

	// Unversioned valid edit on the pre-existing buffer.
	let uri = xeno_lsp::uri_from_path(&path).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range {
					start: lsp_types::Position { line: 0, character: 0 },
					end: lsp_types::Position { line: 0, character: 7 },
				},
				new_text: "updated".into(),
			})],
		}])),
		change_annotations: None,
	};
	editor.apply_workspace_edit(edit).await.unwrap();

	// Pre-existing buffer must remain open with the same identity.
	assert_eq!(
		editor.state.core.editor.buffers.find_by_path(&path),
		Some(view_id),
		"pre-existing buffer must not be closed"
	);
	assert_eq!(buffer_text(&editor, view_id), "updated\n");

	let _ = std::fs::remove_file(path);
}
