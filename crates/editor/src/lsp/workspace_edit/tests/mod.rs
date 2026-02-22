mod atomicity;
mod buffer_identity;
mod coalesce;
mod lifecycle;
mod persistence;
mod range;
mod rename_tracking;
mod version;

use std::path::{Path, PathBuf};

use xeno_lsp::lsp_types;
use xeno_lsp::lsp_types::OptionalVersionedTextDocumentIdentifier;

use super::*;

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

/// Creates a temp file without opening it in the editor.
fn create_temp_file(name: &str, content: &str) -> PathBuf {
	let dir = std::env::temp_dir().join("xeno_test_workspace_edit");
	std::fs::create_dir_all(&dir).unwrap();
	let path = dir.join(name);
	std::fs::write(&path, content).unwrap();
	path
}

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
	Some(first.get("range").is_none())
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
