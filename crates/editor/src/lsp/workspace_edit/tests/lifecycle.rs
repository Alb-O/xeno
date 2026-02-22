use std::path::Path;

use super::*;

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
