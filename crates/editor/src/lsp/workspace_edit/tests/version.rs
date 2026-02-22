use std::path::Path;

use super::*;

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
