use std::path::Path;

use super::*;

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
