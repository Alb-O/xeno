use std::path::Path;

use xeno_lsp::lsp_types;
use xeno_lsp::lsp_types::OptionalVersionedTextDocumentIdentifier;

use super::*;

#[test]
fn workspace_edit_plan_manual_construct() {
	let plan = WorkspaceEditPlan { per_buffer: Vec::new() };
	assert!(plan.affected_buffer_ids().is_empty());
}

#[test]
fn coalesce_rejects_overlap() {
	let mut edits = vec![
		PlannedTextEdit {
			range: 0..2,
			replacement: "a".into(),
		},
		PlannedTextEdit {
			range: 1..3,
			replacement: "b".into(),
		},
	];
	let uri: Uri = "file:///tmp/test.rs".parse().unwrap();
	let err = coalesce_and_validate(&mut edits, &uri).unwrap_err();
	assert!(matches!(err, ApplyError::OverlappingEdits(_)));
}

#[test]
fn convert_text_edit_utf16() {
	let rope = xeno_primitives::Rope::from("a😀b\n");
	let edit = TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 0, character: 1 },
			end: lsp_types::Position { line: 0, character: 3 },
		},
		new_text: "X".into(),
	};
	let planned = convert_text_edit(&rope, OffsetEncoding::Utf16, &edit).unwrap();
	assert_eq!(planned.range.start, 1);
	assert_eq!(planned.range.end, 2);
}

/// Helper: register a document in editor's LSP state and bump its version
/// `n` times. Returns the URI.
fn register_doc_at_version(editor: &crate::Editor, path: &Path, version_bumps: usize) -> Uri {
	let documents = editor.state.lsp.documents();
	let uri = documents.register(path, Some("rust")).unwrap();
	for _ in 0..version_bumps {
		documents.increment_version(&uri);
	}
	uri
}

/// Helper: build a `WorkspaceEdit` with a single `TextDocumentEdit` targeting
/// `uri` at version `v`.
fn versioned_workspace_edit(uri: Uri, version: Option<i32>) -> WorkspaceEdit {
	WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range::default(),
				new_text: "replaced".into(),
			})],
		}])),
		change_annotations: None,
	}
}

#[tokio::test]
async fn workspace_edit_version_mismatch_is_rejected() {
	let mut editor = crate::Editor::new_scratch();
	let path = Path::new("/tmp/version_mismatch.rs");
	let uri = register_doc_at_version(&editor, path, 5);

	// Edit claims version 2, but doc is at version 5.
	let edit = versioned_workspace_edit(uri, Some(2));
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();

	assert!(
		matches!(err, ApplyError::VersionMismatch { expected: 2, actual: 5, .. }),
		"expected VersionMismatch, got: {err:?}"
	);
}

#[tokio::test]
async fn workspace_edit_matching_version_is_not_rejected() {
	let mut editor = crate::Editor::new_scratch();
	let path = Path::new("/tmp/version_match.rs");
	let uri = register_doc_at_version(&editor, path, 3);

	// Version matches — should not produce VersionMismatch.
	// (May fail later for other reasons like BufferNotFound, but not VersionMismatch.)
	let edit = versioned_workspace_edit(uri, Some(3));
	let result = editor.apply_workspace_edit(edit).await;

	match result {
		Err(ApplyError::VersionMismatch { .. }) => panic!("should not reject matching version"),
		_ => {} // any other outcome is fine for this test
	}
}

#[tokio::test]
async fn workspace_edit_none_version_skips_check() {
	let mut editor = crate::Editor::new_scratch();
	let path = Path::new("/tmp/version_none.rs");
	let _uri = register_doc_at_version(&editor, path, 7);

	// version = None → no version check, should not produce VersionMismatch.
	let uri: Uri = "file:///tmp/version_none.rs".parse().unwrap();
	let edit = versioned_workspace_edit(uri, None);
	let result = editor.apply_workspace_edit(edit).await;

	match result {
		Err(ApplyError::VersionMismatch { .. }) => panic!("should not check version when None"),
		_ => {}
	}
}

#[tokio::test]
async fn workspace_edit_untracked_doc_skips_version_check() {
	let mut editor = crate::Editor::new_scratch();

	// URI not registered in LSP state at all.
	let uri: Uri = "file:///tmp/untracked.rs".parse().unwrap();
	let edit = versioned_workspace_edit(uri, Some(42));
	let result = editor.apply_workspace_edit(edit).await;

	match result {
		Err(ApplyError::VersionMismatch { .. }) => panic!("should skip check for untracked docs"),
		_ => {}
	}
}
