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
	let documents = editor.state.integration.lsp.documents();
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
		matches!(err.error, ApplyError::VersionMismatch { expected: 2, actual: 5, .. }),
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
		Err(ref e) if matches!(e.error, ApplyError::VersionMismatch { .. }) => panic!("should not reject matching version"),
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
		Err(ref e) if matches!(e.error, ApplyError::VersionMismatch { .. }) => panic!("should not check version when None"),
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
		matches!(err.error, ApplyError::UntrackedVersionedDocument { version: 42, .. }),
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
		Err(ref e) if matches!(e.error, ApplyError::VersionMismatch { .. } | ApplyError::UntrackedVersionedDocument { .. }) => {
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
	editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(view_id)
		.unwrap()
		.with_doc(|doc| doc.content().to_string())
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

	assert!(
		matches!(err.error, ApplyError::VersionMismatch { .. }),
		"expected VersionMismatch, got: {err:?}"
	);
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
		matches!(err.error, ApplyError::UntrackedVersionedDocument { .. }),
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

/// Creates a temp file without opening it in the editor.
fn create_temp_file(name: &str, content: &str) -> PathBuf {
	let dir = std::env::temp_dir().join("xeno_test_workspace_edit");
	std::fs::create_dir_all(&dir).unwrap();
	let path = dir.join(name);
	std::fs::write(&path, content).unwrap();
	path
}

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

// --- Disk persistence tests ---

#[tokio::test]
async fn workspace_edit_temp_buffer_persists_to_disk_on_success() {
	let mut editor = crate::Editor::new_scratch();
	let path = create_temp_file("persist_success.rs", "old content\n");

	assert!(editor.state.core.editor.buffers.find_by_path(&path).is_none());

	let uri = xeno_lsp::uri_from_path(&path).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range {
					start: lsp_types::Position { line: 0, character: 0 },
					end: lsp_types::Position { line: 0, character: 11 },
				},
				new_text: "new content".into(),
			})],
		}])),
		change_annotations: None,
	};
	editor.apply_workspace_edit(edit).await.unwrap();

	// Buffer should be closed (temp).
	assert!(editor.state.core.editor.buffers.find_by_path(&path).is_none());

	// Disk should have the new content.
	let disk = std::fs::read_to_string(&path).unwrap();
	assert_eq!(disk, "new content\n", "workspace edit must be written to disk for temp buffers");

	let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn workspace_edit_temp_buffer_does_not_write_on_error() {
	let mut editor = crate::Editor::new_scratch();

	// Two files: A gets a valid edit, B has OOB range → error.
	let path_a = create_temp_file("no_write_a.rs", "keep_a\n");
	let path_b = create_temp_file("no_write_b.rs", "keep_b\n");

	let uri_a = xeno_lsp::uri_from_path(&path_a).unwrap();
	let uri_b = xeno_lsp::uri_from_path(&path_b).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_a, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range::default(),
					new_text: "MUTATED_A".into(),
				})],
			},
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_b, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range {
						start: lsp_types::Position { line: 99, character: 0 },
						end: lsp_types::Position { line: 99, character: 5 },
					},
					new_text: "NOPE".into(),
				})],
			},
		])),
		change_annotations: None,
	};
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(matches!(err.error, ApplyError::RangeConversionFailed(_)));

	// No disk writes should have occurred.
	assert_eq!(
		std::fs::read_to_string(&path_a).unwrap(),
		"keep_a\n",
		"file A must be unchanged on disk after error"
	);
	assert_eq!(
		std::fs::read_to_string(&path_b).unwrap(),
		"keep_b\n",
		"file B must be unchanged on disk after error"
	);

	let _ = std::fs::remove_file(path_a);
	let _ = std::fs::remove_file(path_b);
}

/// Two temp files edited successfully → both written to disk atomically,
/// then both buffers closed.
#[tokio::test]
async fn workspace_edit_temp_save_success_closes_all_temps() {
	let mut editor = crate::Editor::new_scratch();
	let path_a = create_temp_file("multi_save_a.rs", "aaa\n");
	let path_b = create_temp_file("multi_save_b.rs", "bbb\n");

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
					range: lsp_types::Range {
						start: lsp_types::Position { line: 0, character: 0 },
						end: lsp_types::Position { line: 0, character: 3 },
					},
					new_text: "AAA".into(),
				})],
			},
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_b, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range {
						start: lsp_types::Position { line: 0, character: 0 },
						end: lsp_types::Position { line: 0, character: 3 },
					},
					new_text: "BBB".into(),
				})],
			},
		])),
		change_annotations: None,
	};
	editor.apply_workspace_edit(edit).await.unwrap();

	// Both temp buffers should be closed.
	assert!(editor.state.core.editor.buffers.find_by_path(&path_a).is_none(), "temp A should be closed");
	assert!(editor.state.core.editor.buffers.find_by_path(&path_b).is_none(), "temp B should be closed");

	// Both files should have new content on disk.
	assert_eq!(std::fs::read_to_string(&path_a).unwrap(), "AAA\n", "file A must be updated on disk");
	assert_eq!(std::fs::read_to_string(&path_b).unwrap(), "BBB\n", "file B must be updated on disk");

	let _ = std::fs::remove_file(path_a);
	let _ = std::fs::remove_file(path_b);
}

/// Two temp files edited, but save fails for one → both buffers remain
/// open (two-phase: no partial close). Disk unchanged for both.
#[cfg(unix)]
#[tokio::test]
async fn workspace_edit_temp_save_failure_keeps_all_temps_alive() {
	use std::os::unix::fs::PermissionsExt;

	let mut editor = crate::Editor::new_scratch();

	// Put each file in its own directory so we can selectively break one.
	let dir_a = std::env::temp_dir().join("xeno_test_save_fail_a");
	let dir_b = std::env::temp_dir().join("xeno_test_save_fail_b");
	std::fs::create_dir_all(&dir_a).unwrap();
	std::fs::create_dir_all(&dir_b).unwrap();
	let path_a = dir_a.join("save_fail_a.rs");
	let path_b = dir_b.join("save_fail_b.rs");
	std::fs::write(&path_a, "aaa\n").unwrap();
	std::fs::write(&path_b, "bbb\n").unwrap();

	assert!(editor.state.core.editor.buffers.find_by_path(&path_a).is_none());
	assert!(editor.state.core.editor.buffers.find_by_path(&path_b).is_none());

	// Make dir_b read-only so atomic write (temp file creation) fails for B.
	std::fs::set_permissions(&dir_b, std::fs::Permissions::from_mode(0o555)).unwrap();

	let uri_a = xeno_lsp::uri_from_path(&path_a).unwrap();
	let uri_b = xeno_lsp::uri_from_path(&path_b).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_a, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range {
						start: lsp_types::Position { line: 0, character: 0 },
						end: lsp_types::Position { line: 0, character: 3 },
					},
					new_text: "AAA".into(),
				})],
			},
			TextDocumentEdit {
				text_document: OptionalVersionedTextDocumentIdentifier { uri: uri_b, version: None },
				edits: vec![OneOf::Left(TextEdit {
					range: lsp_types::Range {
						start: lsp_types::Position { line: 0, character: 0 },
						end: lsp_types::Position { line: 0, character: 3 },
					},
					new_text: "BBB".into(),
				})],
			},
		])),
		change_annotations: None,
	};
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(matches!(err.error, ApplyError::IoWriteFailed { .. }), "expected IoWriteFailed, got: {err:?}");

	// Two-phase semantics: since one save failed, NEITHER buffer is closed.
	assert!(
		editor.state.core.editor.buffers.find_by_path(&path_a).is_some(),
		"temp A must remain open (two-phase: no partial close)"
	);
	assert!(
		editor.state.core.editor.buffers.find_by_path(&path_b).is_some(),
		"temp B must remain open (save failed)"
	);

	// File B's disk content must be unchanged (its write failed).
	assert_eq!(std::fs::read_to_string(&path_b).unwrap(), "bbb\n", "file B must be unchanged on disk");

	// Cleanup: restore permissions.
	std::fs::set_permissions(&dir_b, std::fs::Permissions::from_mode(0o755)).unwrap();
	let _ = std::fs::remove_file(path_a);
	let _ = std::fs::remove_file(path_b);
	let _ = std::fs::remove_dir(dir_a);
	let _ = std::fs::remove_dir(dir_b);
}

/// When two temp buffers resolve to the same canonical path (e.g. via
/// symlink), only one write_atomic call should occur.
#[cfg(unix)]
#[tokio::test]
async fn workspace_edit_temp_save_dedupes_same_target_path() {
	let mut editor = crate::Editor::new_scratch();
	let dir = std::env::temp_dir().join("xeno_test_dedupe_save");
	std::fs::create_dir_all(&dir).unwrap();
	let real_path = dir.join("real.rs");
	let link_path = dir.join("link.rs");
	std::fs::write(&real_path, "original\n").unwrap();

	// Symlink: link.rs → real.rs
	let _ = std::fs::remove_file(&link_path);
	std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

	// Open both paths as separate buffers.
	let view_real = editor.open_file(real_path.clone()).await.unwrap();
	let view_link = editor.open_file(link_path.clone()).await.unwrap();

	// Apply the same edit to both buffers to make them modified.
	{
		use xeno_primitives::{SyntaxPolicy, Transaction, UndoPolicy};

		use crate::buffer::ApplyPolicy;
		let policy = ApplyPolicy {
			undo: UndoPolicy::Record,
			syntax: SyntaxPolicy::IncrementalOrDirty,
		};
		for view_id in [view_real, view_link] {
			let buffer = editor.state.core.editor.buffers.get_buffer_mut(view_id).unwrap();
			let tx = buffer.with_doc(|doc| {
				Transaction::change(
					doc.content().slice(..),
					vec![xeno_primitives::Change {
						start: 0,
						end: 8,
						replacement: Some("modified".into()),
					}],
				)
			});
			buffer.apply(&tx, policy);
		}
	}

	let result = editor.save_temp_buffers_atomic(&[view_real, view_link]).await;
	assert!(result.is_ok(), "identical-bytes symlink dedupe should succeed: {result:?}");

	// Disk should have the new content.
	assert_eq!(std::fs::read_to_string(&real_path).unwrap(), "modified\n");

	let _ = std::fs::remove_file(link_path);
	let _ = std::fs::remove_file(real_path);
	let _ = std::fs::remove_dir(dir);
}

/// A workspace edit targeting a read-only file fails at the buffer
/// apply stage (not the save stage) because the editor marks the
/// buffer as read-only. The file must be unchanged on disk.
#[cfg(unix)]
#[tokio::test]
async fn workspace_edit_readonly_file_rejected() {
	use std::os::unix::fs::PermissionsExt;

	let mut editor = crate::Editor::new_scratch();
	let dir = std::env::temp_dir().join("xeno_test_readonly_edit");
	std::fs::create_dir_all(&dir).unwrap();
	let path = dir.join("readonly.rs");
	std::fs::write(&path, "frozen\n").unwrap();
	std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();

	let uri = xeno_lsp::uri_from_path(&path).unwrap();
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
			text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
			edits: vec![OneOf::Left(TextEdit {
				range: lsp_types::Range {
					start: lsp_types::Position { line: 0, character: 0 },
					end: lsp_types::Position { line: 0, character: 6 },
				},
				new_text: "thawed".into(),
			})],
		}])),
		change_annotations: None,
	};
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(matches!(err.error, ApplyError::ReadOnly(_)), "expected ReadOnly, got: {err:?}");

	// Disk must be unchanged.
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "frozen\n", "read-only file must not be modified");

	// Cleanup.
	std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
	let _ = std::fs::remove_file(path);
	let _ = std::fs::remove_dir(dir);
}

// --- Resource operation tests ---

fn make_temp_dir(label: &str) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("xeno_test_resource_ops_{label}_{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).unwrap();
	dir
}

fn uri_from_path(path: &Path) -> Uri {
	let abs = if path.is_absolute() {
		path.to_path_buf()
	} else {
		std::env::current_dir().unwrap().join(path)
	};
	let url_str = format!("file://{}", abs.display());
	url_str.parse().unwrap()
}

fn create_file_edit(uri: Uri, options: Option<lsp_types::CreateFileOptions>) -> WorkspaceEdit {
	WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Operations(vec![DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
			uri,
			options,
			annotation_id: None,
		}))])),
		change_annotations: None,
	}
}

fn rename_file_edit(old_uri: Uri, new_uri: Uri, options: Option<lsp_types::RenameFileOptions>) -> WorkspaceEdit {
	WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Operations(vec![DocumentChangeOperation::Op(ResourceOp::Rename(RenameFile {
			old_uri,
			new_uri,
			options,
			annotation_id: None,
		}))])),
		change_annotations: None,
	}
}

fn delete_file_edit(uri: Uri, options: Option<lsp_types::DeleteFileOptions>) -> WorkspaceEdit {
	WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Operations(vec![DocumentChangeOperation::Op(ResourceOp::Delete(DeleteFile {
			uri,
			options,
		}))])),
		change_annotations: None,
	}
}

#[tokio::test]
async fn resource_op_create_file_new() {
	let dir = make_temp_dir("create_new");
	let path = dir.join("new_file.rs");
	assert!(!path.exists());

	let mut editor = crate::Editor::new_scratch();
	let edit = create_file_edit(uri_from_path(&path), None);
	editor.apply_workspace_edit(edit).await.unwrap();

	assert!(path.exists());
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "");

	let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn resource_op_create_file_ignore_if_exists() {
	let dir = make_temp_dir("create_ignore");
	let path = dir.join("existing.rs");
	std::fs::write(&path, "original").unwrap();

	let mut editor = crate::Editor::new_scratch();
	let options = lsp_types::CreateFileOptions {
		overwrite: None,
		ignore_if_exists: Some(true),
	};
	let edit = create_file_edit(uri_from_path(&path), Some(options));
	editor.apply_workspace_edit(edit).await.unwrap();

	assert_eq!(std::fs::read_to_string(&path).unwrap(), "original", "file must not be overwritten");

	let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn resource_op_rename_file() {
	let dir = make_temp_dir("rename");
	let old_path = dir.join("old.rs");
	let new_path = dir.join("new.rs");
	std::fs::write(&old_path, "content").unwrap();

	let mut editor = crate::Editor::new_scratch();
	let edit = rename_file_edit(uri_from_path(&old_path), uri_from_path(&new_path), None);
	editor.apply_workspace_edit(edit).await.unwrap();

	assert!(!old_path.exists());
	assert!(new_path.exists());
	assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "content");

	let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn resource_op_delete_file() {
	let dir = make_temp_dir("delete");
	let path = dir.join("to_delete.rs");
	std::fs::write(&path, "gone").unwrap();

	let mut editor = crate::Editor::new_scratch();
	let edit = delete_file_edit(uri_from_path(&path), None);
	editor.apply_workspace_edit(edit).await.unwrap();

	assert!(!path.exists());

	let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn resource_op_delete_ignore_if_missing() {
	let dir = make_temp_dir("delete_missing");
	let path = dir.join("nonexistent.rs");

	let mut editor = crate::Editor::new_scratch();
	let options = lsp_types::DeleteFileOptions {
		recursive: None,
		ignore_if_not_exists: Some(true),
		annotation_id: None,
	};
	let edit = delete_file_edit(uri_from_path(&path), Some(options));
	editor.apply_workspace_edit(edit).await.unwrap();

	let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn resource_op_failed_change_index_reports_first_failure() {
	let dir = make_temp_dir("failed_change");
	let good_path = dir.join("good.rs");
	let bad_path = dir.join("nonexistent.rs");

	let mut editor = crate::Editor::new_scratch();

	// Operation 0: create (should succeed).
	// Operation 1: delete non-existent (should fail).
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Operations(vec![
			DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
				uri: uri_from_path(&good_path),
				options: None,
				annotation_id: None,
			})),
			DocumentChangeOperation::Op(ResourceOp::Delete(DeleteFile {
				uri: uri_from_path(&bad_path),
				options: None,
			})),
		])),
		change_annotations: None,
	};

	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert_eq!(err.failed_change, Some(1), "failed_change must point to the second operation");
	assert!(matches!(err.error, ApplyError::DeleteFailed { .. }));

	// Rollback should have cleaned up the created file.
	assert!(!good_path.exists(), "created file must be rolled back");

	let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn resource_op_rollback_rename_on_failure() {
	let dir = make_temp_dir("rollback_rename");
	let file_a = dir.join("a.rs");
	let file_b = dir.join("b.rs");
	let bad_delete = dir.join("nonexistent.rs");
	std::fs::write(&file_a, "content_a").unwrap();

	let mut editor = crate::Editor::new_scratch();

	// Op 0: rename a→b (succeeds).
	// Op 1: delete nonexistent (fails).
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Operations(vec![
			DocumentChangeOperation::Op(ResourceOp::Rename(RenameFile {
				old_uri: uri_from_path(&file_a),
				new_uri: uri_from_path(&file_b),
				options: None,
				annotation_id: None,
			})),
			DocumentChangeOperation::Op(ResourceOp::Delete(DeleteFile {
				uri: uri_from_path(&bad_delete),
				options: None,
			})),
		])),
		change_annotations: None,
	};

	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert_eq!(err.failed_change, Some(1));

	// Rollback should restore the rename: b→a.
	assert!(file_a.exists(), "renamed file must be restored to original path");
	assert!(!file_b.exists(), "target of rolled-back rename must not exist");
	assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "content_a");

	let _ = std::fs::remove_dir_all(dir);
}

// --- Rename + didChange URI tracking ---

/// Recorded notification with method, URI, and didChange classification.
#[derive(Debug, Clone)]
struct RecordedNotif {
	method: String,
	uri: String,
	/// `Some(true)` = full-text (no range), `Some(false)` = incremental, `None` = non-didChange.
	is_full_change: Option<bool>,
}

/// Whether the transport advertises incremental sync to the server.
#[derive(Debug, Clone, Copy, Default)]
enum SyncMode {
	/// Default: `ServerCapabilities::default()` (no incremental sync).
	#[default]
	Full,
	/// Advertise `TextDocumentSyncKind::INCREMENTAL`.
	Incremental,
}

/// Transport that records notification method + URI for LSP identity assertions.
struct UriRecordingTransport {
	notifications: std::sync::Mutex<Vec<RecordedNotif>>,
	next_slot: std::sync::atomic::AtomicU32,
	sync_mode: SyncMode,
}

impl UriRecordingTransport {
	fn new() -> Self {
		Self {
			notifications: std::sync::Mutex::new(Vec::new()),
			next_slot: std::sync::atomic::AtomicU32::new(1),
			sync_mode: SyncMode::default(),
		}
	}

	fn with_sync_mode(mode: SyncMode) -> Self {
		Self {
			notifications: std::sync::Mutex::new(Vec::new()),
			next_slot: std::sync::atomic::AtomicU32::new(1),
			sync_mode: mode,
		}
	}

	fn recorded(&self) -> Vec<(String, String)> {
		self.notifications.lock().unwrap().iter().map(|n| (n.method.clone(), n.uri.clone())).collect()
	}

	fn recorded_detailed(&self) -> Vec<RecordedNotif> {
		self.notifications.lock().unwrap().clone()
	}

	fn clear_recordings(&self) {
		self.notifications.lock().unwrap().clear();
	}
}

fn classify_notif(method: &str, params: &serde_json::Value) -> Option<bool> {
	if method != "textDocument/didChange" {
		return None;
	}
	let changes = params.get("contentChanges").and_then(|c| c.as_array())?;
	let first = changes.first()?;
	Some(!first.get("range").is_some())
}

#[async_trait::async_trait]
impl xeno_lsp::client::LspTransport for UriRecordingTransport {
	fn subscribe_events(&self) -> xeno_lsp::Result<tokio::sync::mpsc::UnboundedReceiver<xeno_lsp::client::transport::TransportEvent>> {
		let (_, rx) = tokio::sync::mpsc::unbounded_channel();
		Ok(rx)
	}

	async fn start(&self, _cfg: xeno_lsp::client::ServerConfig) -> xeno_lsp::Result<xeno_lsp::client::transport::StartedServer> {
		let slot = self.next_slot.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		Ok(xeno_lsp::client::transport::StartedServer {
			id: xeno_lsp::client::LanguageServerId::new(slot, 0),
		})
	}

	async fn notify(&self, _server: xeno_lsp::client::LanguageServerId, notif: xeno_lsp::AnyNotification) -> xeno_lsp::Result<()> {
		let uri = notif
			.params
			.get("textDocument")
			.and_then(|td| td.get("uri"))
			.and_then(|u| u.as_str())
			.unwrap_or("")
			.to_string();
		let is_full_change = classify_notif(&notif.method, &notif.params);
		self.notifications.lock().unwrap().push(RecordedNotif {
			method: notif.method.clone(),
			uri,
			is_full_change,
		});
		Ok(())
	}

	async fn notify_with_barrier(
		&self,
		server: xeno_lsp::client::LanguageServerId,
		notif: xeno_lsp::AnyNotification,
	) -> xeno_lsp::Result<tokio::sync::oneshot::Receiver<xeno_lsp::Result<()>>> {
		self.notify(server, notif).await?;
		let (tx, rx) = tokio::sync::oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}

	async fn request(
		&self,
		_server: xeno_lsp::client::LanguageServerId,
		_req: xeno_lsp::AnyRequest,
		_timeout: Option<std::time::Duration>,
	) -> xeno_lsp::Result<xeno_lsp::AnyResponse> {
		let capabilities = match self.sync_mode {
			SyncMode::Full => xeno_lsp::lsp_types::ServerCapabilities::default(),
			SyncMode::Incremental => xeno_lsp::lsp_types::ServerCapabilities {
				text_document_sync: Some(xeno_lsp::lsp_types::TextDocumentSyncCapability::Kind(
					xeno_lsp::lsp_types::TextDocumentSyncKind::INCREMENTAL,
				)),
				..Default::default()
			},
		};
		Ok(xeno_lsp::AnyResponse::new_ok(
			xeno_lsp::RequestId::Number(1),
			serde_json::to_value(xeno_lsp::lsp_types::InitializeResult {
				capabilities,
				server_info: None,
			})
			.unwrap(),
		))
	}

	async fn reply(
		&self,
		_server: xeno_lsp::client::LanguageServerId,
		_id: xeno_lsp::RequestId,
		_resp: Result<xeno_lsp::JsonValue, xeno_lsp::ResponseError>,
	) -> xeno_lsp::Result<()> {
		Ok(())
	}

	async fn stop(&self, _server: xeno_lsp::client::LanguageServerId) -> xeno_lsp::Result<()> {
		Ok(())
	}
}

/// After `apply_resource_rename`, the sync manager's tracked config must
/// reflect the new path. Without `maybe_track_lsp_for_buffer(buf_id, true)`
/// after rename, `didChange` would reference the old URI.
#[tokio::test]
async fn resource_op_rename_updates_sync_manager_tracked_path() {
	let dir = make_temp_dir("rename_sync");
	let old_path = dir.join("rename_old.rs");
	let new_path = dir.join("rename_new.rs");
	std::fs::write(&old_path, "fn main() {}\n").unwrap();

	let transport = std::sync::Arc::new(UriRecordingTransport::new());
	let mut editor = crate::Editor::new_scratch_with_transport(transport.clone());

	// Configure a language server for rust so LSP tracking works.
	editor.state.integration.lsp.configure_server(
		"rust",
		crate::lsp::api::LanguageServerConfig {
			command: "rust-analyzer".into(),
			args: vec![],
			env: vec![],
			root_markers: vec![],
			timeout_secs: 30,
			enable_snippets: false,
			initialization_options: None,
			settings: None,
		},
	);
	editor.state.config.lsp_catalog_ready = true;

	// Open the file in the editor.
	let buf_id = editor.open_file(old_path.clone()).await.unwrap();

	// Open the document in LSP directly (simulates what init_lsp_for_open_buffers does).
	let sync = editor.state.integration.lsp.sync().clone();
	let text = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.unwrap()
		.with_doc(|doc| doc.content().clone());
	sync.open_document(&old_path, "rust", &text).await.unwrap();

	// Wait for server initialization.
	let client = editor.state.integration.lsp.registry().get("rust", &old_path).expect("client must exist");
	for _ in 0..200 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "server must be initialized");

	// Track in sync manager and do initial full flush to clear needs_full.
	editor.maybe_track_lsp_for_buffer(buf_id, false);
	let doc_id = editor.state.core.editor.buffers.get_buffer(buf_id).unwrap().document_id();
	let metrics = std::sync::Arc::new(crate::metrics::EditorMetrics::default());
	{
		let snapshot = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(buf_id)
			.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
		let done_rx = editor
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
		if let Some(rx) = done_rx {
			let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		}
	}

	// Rename via the real apply_resource_rename path (uses xeno_lsp::uri_from_path
	// to avoid URI roundtrip mismatches).
	let old_uri = xeno_lsp::uri_from_path(&old_path).unwrap();
	let new_uri = xeno_lsp::uri_from_path(&new_path).unwrap();
	let edit = rename_file_edit(old_uri, new_uri, None);
	editor.apply_workspace_edit(edit).await.unwrap();

	// Clear recordings to isolate post-rename didChange.
	transport.clear_recordings();

	// Apply a local edit.
	{
		let buffer = editor.state.core.editor.buffers.get_buffer_mut(buf_id).unwrap();
		let before = buffer.with_doc(|doc| doc.content().clone());
		let tx = xeno_primitives::Transaction::change(
			before.slice(..),
			vec![xeno_primitives::Change {
				start: 0,
				end: 0,
				replacement: Some("// comment\n".into()),
			}],
		);
		let result = buffer.apply(
			&tx,
			crate::buffer::ApplyPolicy {
				undo: xeno_primitives::UndoPolicy::Record,
				syntax: xeno_primitives::SyntaxPolicy::IncrementalOrDirty,
			},
		);
		editor
			.state
			.integration
			.lsp
			.on_local_edit(editor.state.core.editor.buffers.get_buffer(buf_id).unwrap(), Some(before), &tx, &result);
	}

	// Flush with full snapshot (reset_tracked sets needs_full=true).
	let snapshot = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
	let done_rx = editor
		.state
		.integration
		.lsp
		.sync_manager_mut()
		.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
	if let Some(rx) = done_rx {
		let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		assert!(result.is_ok(), "flush must complete within timeout");
	}

	// Wait for in-flight to drain.
	for _ in 0..200 {
		if editor.state.integration.lsp.sync_manager().in_flight_count() == 0 {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert_eq!(editor.state.integration.lsp.sync_manager().in_flight_count(), 0, "in-flight must drain");

	let recs = transport.recorded();
	let did_changes: Vec<_> = recs.iter().filter(|(m, _)| m == "textDocument/didChange").collect();

	assert!(!did_changes.is_empty(), "expected didChange after rename+edit");
	assert!(
		did_changes.iter().all(|(_, u)| u.contains("rename_new.rs")),
		"all didChange must reference new URI; got: {did_changes:?}",
	);
	assert!(
		did_changes.iter().all(|(_, u)| !u.contains("rename_old.rs")),
		"no didChange must reference old URI; got: {did_changes:?}",
	);

	let _ = std::fs::remove_dir_all(dir);
}

/// Sets up an editor with a recording transport and LSP tracking for a buffer.
/// Returns `(editor, transport, sync, buf_id, doc_id, metrics)`.
/// The document is opened in LSP, server is initialized, sync manager is tracked
/// with an initial full flush completed.
async fn setup_lsp_editor_with_buffer(
	file_path: &Path,
) -> (
	crate::Editor,
	std::sync::Arc<UriRecordingTransport>,
	xeno_lsp::DocumentSync,
	ViewId,
	xeno_primitives::DocumentId,
	std::sync::Arc<crate::metrics::EditorMetrics>,
) {
	let transport = std::sync::Arc::new(UriRecordingTransport::new());
	let mut editor = crate::Editor::new_scratch_with_transport(transport.clone());

	editor.state.integration.lsp.configure_server(
		"rust",
		crate::lsp::api::LanguageServerConfig {
			command: "rust-analyzer".into(),
			args: vec![],
			env: vec![],
			root_markers: vec![],
			timeout_secs: 30,
			enable_snippets: false,
			initialization_options: None,
			settings: None,
		},
	);
	editor.state.config.lsp_catalog_ready = true;

	let buf_id = editor.open_file(file_path.to_path_buf()).await.unwrap();
	editor.focus_buffer(buf_id);

	let sync = editor.state.integration.lsp.sync().clone();
	let text = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.unwrap()
		.with_doc(|doc| doc.content().clone());
	sync.open_document(file_path, "rust", &text).await.unwrap();

	let client = editor.state.integration.lsp.registry().get("rust", file_path).expect("client must exist");
	for _ in 0..200 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "server must be initialized");

	editor.maybe_track_lsp_for_buffer(buf_id, false);
	let doc_id = editor.state.core.editor.buffers.get_buffer(buf_id).unwrap().document_id();
	let metrics = std::sync::Arc::new(crate::metrics::EditorMetrics::default());

	// Initial full flush to clear needs_full.
	{
		let snapshot = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(buf_id)
			.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
		let done_rx = editor
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
		if let Some(rx) = done_rx {
			let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		}
	}

	(editor, transport, sync, buf_id, doc_id, metrics)
}

/// Applies a local edit to the buffer and flushes the sync manager.
/// Returns the recorded notifications after the flush.
async fn edit_and_flush(
	editor: &mut crate::Editor,
	transport: &UriRecordingTransport,
	sync: &xeno_lsp::DocumentSync,
	buf_id: ViewId,
	doc_id: xeno_primitives::DocumentId,
	metrics: &std::sync::Arc<crate::metrics::EditorMetrics>,
) -> Vec<(String, String)> {
	transport.clear_recordings();

	{
		let buffer = editor.state.core.editor.buffers.get_buffer_mut(buf_id).unwrap();
		let before = buffer.with_doc(|doc| doc.content().clone());
		let tx = xeno_primitives::Transaction::change(
			before.slice(..),
			vec![xeno_primitives::Change {
				start: 0,
				end: 0,
				replacement: Some("// edit\n".into()),
			}],
		);
		let result = buffer.apply(
			&tx,
			crate::buffer::ApplyPolicy {
				undo: xeno_primitives::UndoPolicy::Record,
				syntax: xeno_primitives::SyntaxPolicy::IncrementalOrDirty,
			},
		);
		editor
			.state
			.integration
			.lsp
			.on_local_edit(editor.state.core.editor.buffers.get_buffer(buf_id).unwrap(), Some(before), &tx, &result);
	}

	let snapshot = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
	let done_rx = editor
		.state
		.integration
		.lsp
		.sync_manager_mut()
		.flush_now(std::time::Instant::now(), doc_id, sync, metrics, snapshot);
	if let Some(rx) = done_rx {
		let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		assert!(result.is_ok(), "flush must complete within timeout");
	}

	for _ in 0..200 {
		if editor.state.integration.lsp.sync_manager().in_flight_count() == 0 {
			break;
		}
		tokio::task::yield_now().await;
	}

	transport.recorded()
}

/// After a buffer path change + reset_tracked (as save-as does), didChange
/// must target the new URI.
#[tokio::test]
async fn save_as_updates_sync_manager_tracked_path() {
	let dir = make_temp_dir("save_as_sync");
	let old_path = dir.join("save_old.rs");
	let new_path = dir.join("save_new.rs");
	std::fs::write(&old_path, "fn main() {}\n").unwrap();

	let (mut editor, transport, sync, buf_id, doc_id, metrics) = setup_lsp_editor_with_buffer(&old_path).await;

	// Simulate save-as: change buffer path + reset LSP tracking.
	// (save_as() operates on the focused buffer, which requires window layout
	// plumbing; here we exercise the same code path directly.)
	let loader = editor.state.config.config.language_loader.clone();
	editor
		.state
		.core
		.editor
		.buffers
		.get_buffer_mut(buf_id)
		.unwrap()
		.set_path(Some(new_path.clone()), Some(&loader));
	editor.maybe_track_lsp_for_buffer(buf_id, true);

	// Edit + flush. After path change, reset_tracked sets needs_full=true.
	// The sync manager sends a full-text sync which, because the new URI isn't
	// opened yet, goes through the open_if_needed path and emits didOpen (not
	// didChange) with the full text at the new URI.
	let recs = edit_and_flush(&mut editor, &transport, &sync, buf_id, doc_id, &metrics).await;

	// Filter for notifications that carry the document's text (didOpen or didChange).
	let doc_notifications: Vec<_> = recs
		.iter()
		.filter(|(m, _)| m == "textDocument/didOpen" || m == "textDocument/didChange")
		.collect();

	assert!(
		!doc_notifications.is_empty(),
		"expected didOpen or didChange after save-as + edit; got: {recs:?}"
	);
	assert!(
		doc_notifications.iter().all(|(_, u)| u.contains("save_new.rs")),
		"all doc notifications must reference new URI; got: {doc_notifications:?}",
	);
	assert!(
		doc_notifications.iter().all(|(_, u)| !u.contains("save_old.rs")),
		"no doc notifications must reference old URI; got: {doc_notifications:?}",
	);

	let _ = std::fs::remove_dir_all(dir);
}

/// After a rename is rolled back, didChange must target the restored (original) URI.
#[tokio::test]
async fn rename_rollback_restores_sync_manager_tracked_path() {
	let dir = make_temp_dir("rollback_sync");
	let old_path = dir.join("rollback_old.rs");
	let renamed_path = dir.join("rollback_renamed.rs");
	let nonexistent = dir.join("nonexistent.rs");
	std::fs::write(&old_path, "fn main() {}\n").unwrap();

	let (mut editor, transport, sync, buf_id, doc_id, metrics) = setup_lsp_editor_with_buffer(&old_path).await;

	// Op 0: rename old → renamed (succeeds).
	// Op 1: delete nonexistent (fails, triggers rollback of rename).
	let edit = WorkspaceEdit {
		changes: None,
		document_changes: Some(DocumentChanges::Operations(vec![
			DocumentChangeOperation::Op(ResourceOp::Rename(RenameFile {
				old_uri: xeno_lsp::uri_from_path(&old_path).unwrap(),
				new_uri: xeno_lsp::uri_from_path(&renamed_path).unwrap(),
				options: None,
				annotation_id: None,
			})),
			DocumentChangeOperation::Op(ResourceOp::Delete(DeleteFile {
				uri: uri_from_path(&nonexistent),
				options: None,
			})),
		])),
		change_annotations: None,
	};
	let err = editor.apply_workspace_edit(edit).await.unwrap_err();
	assert!(matches!(err.error, ApplyError::DeleteFailed { .. }));

	// After rollback, buffer should be back at old path.
	assert!(old_path.exists(), "file must be restored to original path");

	// Edit + flush, assert didChange targets the restored original URI.
	let recs = edit_and_flush(&mut editor, &transport, &sync, buf_id, doc_id, &metrics).await;
	let did_changes: Vec<_> = recs.iter().filter(|(m, _)| m == "textDocument/didChange").collect();

	assert!(!did_changes.is_empty(), "expected didChange after rollback + edit");
	assert!(
		did_changes.iter().all(|(_, u)| u.contains("rollback_old.rs")),
		"all didChange must reference restored URI; got: {did_changes:?}",
	);
	assert!(
		did_changes.iter().all(|(_, u)| !u.contains("rollback_renamed.rs")),
		"no didChange must reference renamed URI; got: {did_changes:?}",
	);

	let _ = std::fs::remove_dir_all(dir);
}

// --- LSP lifecycle golden tests ---

/// After open + edit + flush with incremental sync capabilities, the transport
/// must show `didChange(incremental)` with the correct URI and no spurious `didOpen`.
#[tokio::test]
async fn lifecycle_open_edit_flush_produces_incremental_change() {
	let dir = make_temp_dir("lifecycle_incr");
	let path = dir.join("incr_edit.rs");
	std::fs::write(&path, "fn main() {}\n").unwrap();

	// Use incremental sync mode so the sync manager sends ranged changes.
	let transport = std::sync::Arc::new(UriRecordingTransport::with_sync_mode(SyncMode::Incremental));
	let mut editor = crate::Editor::new_scratch_with_transport(transport.clone());

	editor.state.integration.lsp.configure_server(
		"rust",
		crate::lsp::api::LanguageServerConfig {
			command: "rust-analyzer".into(),
			args: vec![],
			env: vec![],
			root_markers: vec![],
			timeout_secs: 30,
			enable_snippets: false,
			initialization_options: None,
			settings: None,
		},
	);
	editor.state.config.lsp_catalog_ready = true;

	let buf_id = editor.open_file(path.clone()).await.unwrap();
	editor.focus_buffer(buf_id);

	let sync = editor.state.integration.lsp.sync().clone();
	let text = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.unwrap()
		.with_doc(|doc| doc.content().clone());
	sync.open_document(&path, "rust", &text).await.unwrap();

	let client = editor.state.integration.lsp.registry().get("rust", &path).expect("client must exist");
	for _ in 0..200 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "server must be initialized");

	editor.maybe_track_lsp_for_buffer(buf_id, false);
	let doc_id = editor.state.core.editor.buffers.get_buffer(buf_id).unwrap().document_id();
	let metrics = std::sync::Arc::new(crate::metrics::EditorMetrics::default());

	// Initial flush to clear needs_full.
	{
		let snapshot = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(buf_id)
			.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
		let done_rx = editor
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
		if let Some(rx) = done_rx {
			let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		}
	}

	// Clear recordings from initial setup.
	transport.clear_recordings();

	// Edit + flush.
	let _recs = edit_and_flush(&mut editor, &transport, &sync, buf_id, doc_id, &metrics).await;

	let detailed = transport.recorded_detailed();
	let did_changes: Vec<_> = detailed.iter().filter(|n| n.method == "textDocument/didChange").collect();

	assert!(!did_changes.is_empty(), "expected at least one didChange; got: {detailed:?}");

	let expected_uri = xeno_lsp::uri_from_path(&path).unwrap().to_string();
	for dc in &did_changes {
		assert_eq!(dc.uri, expected_uri, "didChange URI mismatch");
		assert_eq!(
			dc.is_full_change,
			Some(false),
			"didChange must be incremental with INCREMENTAL caps; got: {dc:?}"
		);
	}

	// No didOpen in this window (doc was already open from setup).
	let did_opens: Vec<_> = detailed.iter().filter(|n| n.method == "textDocument/didOpen").collect();
	assert!(did_opens.is_empty(), "no didOpen expected after initial open; got: {did_opens:?}");

	let _ = std::fs::remove_dir_all(dir);
}

/// Verify the full open lifecycle: `didOpen` precedes any `didChange`.
#[tokio::test]
async fn lifecycle_did_open_precedes_did_change() {
	let dir = make_temp_dir("lifecycle_order");
	let path = dir.join("order.rs");
	std::fs::write(&path, "fn main() {}\n").unwrap();

	let transport = std::sync::Arc::new(UriRecordingTransport::new());
	let mut editor = crate::Editor::new_scratch_with_transport(transport.clone());

	editor.state.integration.lsp.configure_server(
		"rust",
		crate::lsp::api::LanguageServerConfig {
			command: "rust-analyzer".into(),
			args: vec![],
			env: vec![],
			root_markers: vec![],
			timeout_secs: 30,
			enable_snippets: false,
			initialization_options: None,
			settings: None,
		},
	);
	editor.state.config.lsp_catalog_ready = true;

	let buf_id = editor.open_file(path.clone()).await.unwrap();
	editor.focus_buffer(buf_id);

	let sync = editor.state.integration.lsp.sync().clone();
	let text = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.unwrap()
		.with_doc(|doc| doc.content().clone());
	sync.open_document(&path, "rust", &text).await.unwrap();

	let client = editor.state.integration.lsp.registry().get("rust", &path).expect("client must exist");
	for _ in 0..200 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "server must be initialized");

	editor.maybe_track_lsp_for_buffer(buf_id, false);
	let doc_id = editor.state.core.editor.buffers.get_buffer(buf_id).unwrap().document_id();
	let metrics = std::sync::Arc::new(crate::metrics::EditorMetrics::default());

	// Do NOT clear recordings — we want the full history including didOpen.
	// Initial flush.
	{
		let snapshot = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(buf_id)
			.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
		let done_rx = editor
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
		if let Some(rx) = done_rx {
			let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		}
	}

	// Edit + flush.
	{
		let buffer = editor.state.core.editor.buffers.get_buffer_mut(buf_id).unwrap();
		let before = buffer.with_doc(|doc| doc.content().clone());
		let tx = xeno_primitives::Transaction::change(
			before.slice(..),
			vec![xeno_primitives::Change {
				start: 0,
				end: 0,
				replacement: Some("// comment\n".into()),
			}],
		);
		let result = buffer.apply(
			&tx,
			crate::buffer::ApplyPolicy {
				undo: xeno_primitives::UndoPolicy::Record,
				syntax: xeno_primitives::SyntaxPolicy::IncrementalOrDirty,
			},
		);
		editor
			.state
			.integration
			.lsp
			.on_local_edit(editor.state.core.editor.buffers.get_buffer(buf_id).unwrap(), Some(before), &tx, &result);
	}

	{
		let snapshot = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(buf_id)
			.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
		let done_rx = editor
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
		if let Some(rx) = done_rx {
			let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		}
	}

	let detailed = transport.recorded_detailed();
	let expected_uri = xeno_lsp::uri_from_path(&path).unwrap().to_string();

	let open_idx = detailed.iter().position(|n| n.method == "textDocument/didOpen" && n.uri == expected_uri);
	let change_idx = detailed.iter().position(|n| n.method == "textDocument/didChange" && n.uri == expected_uri);

	assert!(open_idx.is_some(), "didOpen must be present; all notifs: {detailed:?}");
	if let Some(ci) = change_idx {
		assert!(
			open_idx.unwrap() < ci,
			"didOpen must precede didChange; open_idx={}, change_idx={}; notifs: {detailed:?}",
			open_idx.unwrap(),
			ci
		);
	}

	let _ = std::fs::remove_dir_all(dir);
}

/// After closing a buffer's LSP tracking, `didChange` traffic must not
/// reference the closed document's URI without a preceding reopen (`didOpen`).
#[tokio::test]
async fn lifecycle_close_stops_further_traffic() {
	let dir = make_temp_dir("lifecycle_close");
	let path = dir.join("close_test.rs");
	std::fs::write(&path, "fn main() {}\n").unwrap();

	let (mut editor, transport, sync, buf_id, doc_id, metrics) = setup_lsp_editor_with_buffer(&path).await;

	// Edit and flush to confirm traffic flows pre-close.
	let recs = edit_and_flush(&mut editor, &transport, &sync, buf_id, doc_id, &metrics).await;
	assert!(!recs.is_empty(), "pre-close edit must produce notifications");

	// Close the document in LSP.
	let lang = editor
		.state
		.core
		.editor
		.buffers
		.get_buffer(buf_id)
		.and_then(|b| b.file_type().map(|s| s.to_string()))
		.unwrap_or_else(|| "rust".to_string());
	sync.close_document(&path, &lang).await.unwrap();

	// Clear and try another flush cycle.
	transport.clear_recordings();

	// Attempt another edit + flush (buffer still exists in editor, but LSP doc is closed).
	{
		let buffer = editor.state.core.editor.buffers.get_buffer_mut(buf_id).unwrap();
		let before = buffer.with_doc(|doc| doc.content().clone());
		let tx = xeno_primitives::Transaction::change(
			before.slice(..),
			vec![xeno_primitives::Change {
				start: 0,
				end: 0,
				replacement: Some("// post-close\n".into()),
			}],
		);
		let result = buffer.apply(
			&tx,
			crate::buffer::ApplyPolicy {
				undo: xeno_primitives::UndoPolicy::Record,
				syntax: xeno_primitives::SyntaxPolicy::IncrementalOrDirty,
			},
		);
		editor
			.state
			.integration
			.lsp
			.on_local_edit(editor.state.core.editor.buffers.get_buffer(buf_id).unwrap(), Some(before), &tx, &result);
	}

	{
		let snapshot = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(buf_id)
			.map(|b| b.with_doc(|doc| (doc.content().clone(), doc.version())));
		let done_rx = editor
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(std::time::Instant::now(), doc_id, &sync, &metrics, snapshot);
		if let Some(rx) = done_rx {
			let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
		}
	}

	let post_close = transport.recorded_detailed();
	let expected_uri = xeno_lsp::uri_from_path(&path).unwrap().to_string();

	// After close_document, any didChange must be preceded by didOpen (reopen).
	let stale_changes: Vec<_> = post_close
		.iter()
		.filter(|n| n.method == "textDocument/didChange" && n.uri == expected_uri)
		.collect();

	if !stale_changes.is_empty() {
		let reopen = post_close.iter().position(|n| n.method == "textDocument/didOpen" && n.uri == expected_uri);
		let first_change = post_close.iter().position(|n| n.method == "textDocument/didChange" && n.uri == expected_uri);
		assert!(
			reopen.is_some() && reopen.unwrap() < first_change.unwrap(),
			"didChange after close must be preceded by didOpen (reopen); notifs: {post_close:?}"
		);
	}

	let _ = std::fs::remove_dir_all(dir);
}
