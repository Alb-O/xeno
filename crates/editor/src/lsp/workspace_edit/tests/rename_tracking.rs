use super::*;

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
