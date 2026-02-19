use std::path::{Path, PathBuf};

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
async fn workspace_edit_versioned_untracked_doc_is_rejected() {
	let mut editor = crate::Editor::new_scratch();

	// URI not registered in LSP state at all, but edit carries a version.
	let uri: Uri = "file:///tmp/untracked.rs".parse().unwrap();
	let edit = versioned_workspace_edit(uri, Some(42));
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();

	assert!(
		matches!(err, ApplyError::UntrackedVersionedDocument { version: 42, .. }),
		"expected UntrackedVersionedDocument, got: {err:?}"
	);
}

#[tokio::test]
async fn workspace_edit_unversioned_untracked_doc_skips_check() {
	let mut editor = crate::Editor::new_scratch();

	// URI not registered, but edit has no version — no version check.
	let uri: Uri = "file:///tmp/untracked_unversioned.rs".parse().unwrap();
	let edit = versioned_workspace_edit(uri, None);
	let result = editor.apply_workspace_edit(edit).await;

	match result {
		Err(ApplyError::VersionMismatch { .. } | ApplyError::UntrackedVersionedDocument { .. }) => {
			panic!("should skip version check for unversioned edits")
		}
		_ => {}
	}
}

/// Helper: build a multi-doc `WorkspaceEdit` with `TextDocumentEdit` entries.
fn multi_doc_workspace_edit(entries: Vec<(Uri, Option<i32>, &str)>) -> WorkspaceEdit {
	let edits = entries
		.into_iter()
		.map(|(uri, version, new_text)| TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range::default(),
				new_text: new_text.into(),
			})],
		})
		.collect();
	WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(edits)),
		change_annotations: None,
	}
}

/// Creates a temp file with given content, opens it in the editor, and
/// registers the path in LSP state at the given version. Returns the
/// (path, uri, view_id).
async fn open_temp_doc(editor: &mut crate::Editor, name: &str, content: &str, version: i32) -> (PathBuf, Uri, ViewId) {
	let dir = std::env::temp_dir().join("xeno_test_workspace_edit");
	std::fs::create_dir_all(&dir).unwrap();
	let path = dir.join(name);
	std::fs::write(&path, content).unwrap();

	let view_id = editor.open_file(path.clone()).await.unwrap();
	let uri = register_doc_at_version(editor, &path, version as usize);
	(path, uri, view_id)
}

fn buffer_text(editor: &crate::Editor, view_id: ViewId) -> String {
	editor.state.core.buffers.get_buffer(view_id).unwrap().with_doc(|doc| doc.content().to_string())
}

#[tokio::test]
async fn workspace_edit_multi_doc_mismatch_does_not_partially_apply() {
	let mut editor = crate::Editor::new_scratch();

	// Doc A: version matches. Doc B: version mismatch.
	// Doc B appears second in the edit list so Doc A's edits are collected
	// before the version check on Doc B rejects the entire edit.
	let (path_a, uri_a, view_a) = open_temp_doc(&mut editor, "atomic_a.rs", "original_a\n", 3).await;
	let (path_b, uri_b, _view_b) = open_temp_doc(&mut editor, "atomic_b.rs", "original_b\n", 5).await;

	let edit = multi_doc_workspace_edit(vec![
		(uri_a.clone(), Some(3), "MUTATED_A"),
		(uri_b.clone(), Some(1), "MUTATED_B"), // stale version
	]);
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();

	assert!(matches!(err, ApplyError::VersionMismatch { .. }), "expected VersionMismatch, got: {err:?}");
	assert_eq!(buffer_text(&editor, view_a), "original_a\n", "Doc A must be unchanged after rejected edit");

	// Cleanup.
	let _ = std::fs::remove_file(path_a);
	let _ = std::fs::remove_file(path_b);
}

#[tokio::test]
async fn workspace_edit_multi_doc_untracked_does_not_partially_apply() {
	let mut editor = crate::Editor::new_scratch();

	// Doc A: tracked + version matches. Doc B: versioned but untracked.
	let (path_a, uri_a, view_a) = open_temp_doc(&mut editor, "atomic_tracked.rs", "original_tracked\n", 2).await;
	let uri_b: Uri = "file:///tmp/xeno_test_not_tracked.rs".parse().unwrap();

	let edit = multi_doc_workspace_edit(vec![
		(uri_a.clone(), Some(2), "MUTATED_TRACKED"),
		(uri_b, Some(99), "MUTATED_UNTRACKED"), // not in LSP state
	]);
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();

	assert!(
		matches!(err, ApplyError::UntrackedVersionedDocument { .. }),
		"expected UntrackedVersionedDocument, got: {err:?}"
	);
	assert_eq!(
		buffer_text(&editor, view_a),
		"original_tracked\n",
		"Doc A must be unchanged after rejected edit"
	);

	let _ = std::fs::remove_file(path_a);
}

// --- Range sanity tests ---

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
	let (path, uri, _view_id) = open_temp_doc(&mut editor, "range_invalid.rs", "hello\nworld\n", 0).await;

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
		matches!(err, ApplyError::RangeConversionFailed(_)),
		"expected RangeConversionFailed, got: {err:?}"
	);

	// resolve_uri_to_buffer reopens the buffer, so look it up by path.
	let current_view = editor.state.core.buffers.find_by_path(&path).expect("buffer should still exist");
	assert_eq!(buffer_text(&editor, current_view), "hello\nworld\n", "buffer must be unchanged");

	let _ = std::fs::remove_file(path);
}
