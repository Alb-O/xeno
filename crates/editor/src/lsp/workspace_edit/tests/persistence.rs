use std::path::Path;

use super::*;

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
