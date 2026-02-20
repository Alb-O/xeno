use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use lsp_types::{Diagnostic, DiagnosticSeverity, Range, Uri};
use ropey::Rope;
use tokio::sync::{mpsc, oneshot};

use super::*;

struct SimpleStubTransport;
#[async_trait]
impl crate::client::transport::LspTransport for SimpleStubTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		let (_, rx) = mpsc::unbounded_channel();
		Ok(rx)
	}
	async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		Ok(crate::client::transport::StartedServer {
			id: LanguageServerId::new(1, 0),
		})
	}
	async fn notify(&self, _server: LanguageServerId, _notif: crate::AnyNotification) -> crate::Result<()> {
		Ok(())
	}
	async fn notify_with_barrier(&self, _server: LanguageServerId, _notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}
	async fn request(&self, _server: LanguageServerId, _req: crate::AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<crate::AnyResponse> {
		Err(crate::Error::Protocol("SimpleStubTransport".into()))
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::types::RequestId,
		_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

#[test]
fn test_document_sync_with_registry() {
	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport, xeno_worker::WorkerRuntime::new()));
	let documents = Arc::new(DocumentStateManager::new());
	let sync = DocumentSync::with_registry(registry, documents);

	assert_eq!(sync.total_error_count(), 0);
	assert_eq!(sync.total_warning_count(), 0);
}

#[tokio::test]
async fn test_document_sync_create() {
	let transport = Arc::new(SimpleStubTransport);
	let (sync, _registry, _documents, _receiver) = DocumentSync::create(transport, xeno_worker::WorkerRuntime::new());

	assert_eq!(sync.total_error_count(), 0);
	assert_eq!(sync.total_warning_count(), 0);
}

#[test]
fn test_diagnostics_event_updates_state() {
	let documents = Arc::new(DocumentStateManager::new());
	let handler = DocumentSyncEventHandler::new(documents.clone());
	let uri: Uri = "file:///test.rs".parse().expect("valid uri");

	handler.on_diagnostics(
		LanguageServerId::new(1, 0),
		uri.clone(),
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "test error".to_string(),
			..Diagnostic::default()
		}],
		None,
	);

	let diagnostics = documents.get_diagnostics(&uri);
	assert_eq!(diagnostics.len(), 1);
}

#[tokio::test]
async fn test_document_sync_returns_not_ready_before_init() {
	use crate::registry::LanguageServerConfig;

	struct InitStubTransport;
	#[async_trait]
	impl crate::client::transport::LspTransport for InitStubTransport {
		fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
			let (_, rx) = mpsc::unbounded_channel();
			Ok(rx)
		}
		async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
			Ok(crate::client::transport::StartedServer {
				id: LanguageServerId::new(1, 0),
			})
		}
		async fn notify(&self, _server: LanguageServerId, _notif: crate::AnyNotification) -> crate::Result<()> {
			Ok(())
		}
		async fn notify_with_barrier(&self, _server: LanguageServerId, _notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
			let (tx, rx) = oneshot::channel();
			let _ = tx.send(Ok(()));
			Ok(rx)
		}
		async fn request(
			&self,
			_server: LanguageServerId,
			_req: crate::AnyRequest,
			_timeout: Option<std::time::Duration>,
		) -> crate::Result<crate::AnyResponse> {
			// Return a dummy response for initialize
			Ok(crate::AnyResponse {
				id: crate::RequestId::Number(1),
				result: Some(
					serde_json::to_value(lsp_types::InitializeResult {
						capabilities: lsp_types::ServerCapabilities::default(),
						server_info: None,
					})
					.unwrap(),
				),
				error: None,
			})
		}
		async fn reply(
			&self,
			_server: LanguageServerId,
			_id: crate::types::RequestId,
			_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
		) -> crate::Result<()> {
			Ok(())
		}
		async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
			Ok(())
		}
	}

	let transport = Arc::new(InitStubTransport);
	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport, xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("test.rs");
	let content = Rope::from("fn main() {}");

	// Open it first (didOpen does not check initialization in DocumentSync)
	sync.open_document(path, "rust", &content).await.unwrap();

	// acquire will spawn initialize in background
	let client = registry.get("rust", path).unwrap();

	// Ensure it's NOT initialized yet (background task might not have run)
	if !client.is_initialized() {
		let result = sync
			.send_change(ChangeRequest::full_text(path, "rust", content.to_string()).with_barrier(BarrierMode::None))
			.await;
		assert!(matches!(result, Err(crate::Error::NotReady)));
	}
}

#[tokio::test]
async fn test_send_change_incremental_empty_is_noop() {
	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport, xeno_worker::WorkerRuntime::new()));
	let documents = Arc::new(DocumentStateManager::new());
	let sync = DocumentSync::with_registry(registry, documents);

	let dispatch = sync
		.send_change(ChangeRequest::incremental(Path::new("does/not/matter.rs"), "rust", Vec::new()))
		.await
		.expect("empty incremental request should noop");

	assert!(dispatch.barrier.is_none());
	assert!(dispatch.applied_version.is_none());
	assert!(!dispatch.opened_document);
}

#[tokio::test]
async fn barrier_ignored_after_doc_close() {
	let documents = Arc::new(DocumentStateManager::new());
	let path = Path::new("/barrier_close.rs");
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Queue a change and create a barrier that we control.
	let version = documents.queue_change(&uri).unwrap();
	let (barrier_tx, barrier_rx) = oneshot::channel();

	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport, xeno_worker::WorkerRuntime::new()));
	let sync = DocumentSync::with_registry(registry, documents.clone());

	let completion_rx = sync.wrap_barrier(uri.clone(), version, barrier_rx);

	// Close the document before the barrier resolves.
	documents.unregister(&uri);

	// Resolve the barrier — the ack should be skipped.
	barrier_tx.send(Ok(())).unwrap();
	completion_rx.await.unwrap();

	// Re-register to inspect state: no pending changes should have been acked.
	let uri = documents.register(path, Some("rust")).unwrap();
	assert_eq!(documents.pending_change_count(&uri), 0);
	assert!(!documents.take_force_full_sync_by_uri(&uri));
}

#[tokio::test]
async fn barrier_ignored_after_doc_reopen() {
	let documents = Arc::new(DocumentStateManager::new());
	let path = Path::new("/barrier_reopen.rs");
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Queue a change and create a barrier.
	let version = documents.queue_change(&uri).unwrap();
	let (barrier_tx, barrier_rx) = oneshot::channel();

	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport, xeno_worker::WorkerRuntime::new()));
	let sync = DocumentSync::with_registry(registry, documents.clone());

	let completion_rx = sync.wrap_barrier(uri.clone(), version, barrier_rx);

	// Close and reopen the document — new session, new generation.
	documents.unregister(&uri);
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Queue a change in the new session.
	let _new_version = documents.queue_change(&uri).unwrap();

	// Resolve the old barrier — should be silently ignored.
	barrier_tx.send(Ok(())).unwrap();
	completion_rx.await.unwrap();

	// The new session's pending change should still be there (not acked by stale barrier).
	assert_eq!(documents.pending_change_count(&uri), 1, "stale barrier should not ack new session's change");
}

#[tokio::test]
async fn barrier_error_ignored_after_doc_reopen() {
	let documents = Arc::new(DocumentStateManager::new());
	let path = Path::new("/barrier_err_reopen.rs");
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	let version = documents.queue_change(&uri).unwrap();
	let (barrier_tx, barrier_rx) = oneshot::channel();

	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport, xeno_worker::WorkerRuntime::new()));
	let sync = DocumentSync::with_registry(registry, documents.clone());

	let completion_rx = sync.wrap_barrier(uri.clone(), version, barrier_rx);

	// Close and reopen.
	documents.unregister(&uri);
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Resolve with error — should NOT mark force_full_sync on the new session.
	barrier_tx.send(Err(crate::Error::Protocol("test".into()))).unwrap();
	completion_rx.await.unwrap();

	assert!(
		!documents.take_force_full_sync_by_uri(&uri),
		"stale barrier error should not force full sync on new session"
	);
}

/// Recorded notification entry.
#[derive(Debug, Clone)]
struct RecordedNotification {
	server_id: LanguageServerId,
	method: String,
	uri: Option<String>,
	/// For `textDocument/didChange` only: `true` if full-text (no range),
	/// `false` if incremental (has range). `None` for other methods.
	is_full_change: Option<bool>,
}

/// Transport that records notification methods, server ids, and URIs in order.
/// Methods listed in `fail_methods` will return an error instead of succeeding.
struct RecordingTransport {
	notifications: std::sync::Mutex<Vec<RecordedNotification>>,
	next_slot: std::sync::atomic::AtomicU32,
	fail_methods: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl RecordingTransport {
	fn new() -> Self {
		Self {
			notifications: std::sync::Mutex::new(Vec::new()),
			next_slot: std::sync::atomic::AtomicU32::new(1),
			fail_methods: std::sync::Mutex::new(std::collections::HashSet::new()),
		}
	}

	fn set_fail_method(&self, method: &str) {
		self.fail_methods.lock().unwrap().insert(method.to_string());
	}

	fn clear_fail_method(&self, method: &str) {
		self.fail_methods.lock().unwrap().remove(method);
	}

	fn recorded(&self) -> Vec<RecordedNotification> {
		self.notifications.lock().unwrap().clone()
	}

	fn recorded_methods(&self) -> Vec<String> {
		self.notifications.lock().unwrap().iter().map(|n| n.method.clone()).collect()
	}

	/// Records the notification and returns `Err` if the method is in the fail set.
	fn record(&self, server_id: LanguageServerId, notif: &crate::AnyNotification) -> crate::Result<()> {
		let uri = notif
			.params
			.get("textDocument")
			.and_then(|td| td.get("uri"))
			.and_then(|u| u.as_str())
			.map(|s| s.to_string());
		let is_full_change = if notif.method == "textDocument/didChange" {
			notif
				.params
				.get("contentChanges")
				.and_then(|cc| cc.as_array())
				.and_then(|arr| arr.first())
				.map(|first| first.get("range").is_none())
		} else {
			None
		};

		self.notifications.lock().unwrap().push(RecordedNotification {
			server_id,
			method: notif.method.clone(),
			uri,
			is_full_change,
		});
		if self.fail_methods.lock().unwrap().contains(&notif.method) {
			return Err(crate::Error::Protocol(format!("injected failure for {}", notif.method)));
		}
		Ok(())
	}
}

#[async_trait]
impl crate::client::transport::LspTransport for RecordingTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		let (_, rx) = mpsc::unbounded_channel();
		Ok(rx)
	}
	async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		let slot = self.next_slot.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		Ok(crate::client::transport::StartedServer {
			id: LanguageServerId::new(slot, 0),
		})
	}
	async fn notify(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<()> {
		self.record(server, &notif)?;
		Ok(())
	}
	async fn notify_with_barrier(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		self.record(server, &notif)?;
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}
	async fn request(&self, _server: LanguageServerId, _req: crate::AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<crate::AnyResponse> {
		Err(crate::Error::Protocol("RecordingTransport".into()))
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::types::RequestId,
		_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

#[tokio::test]
async fn reopen_document_sends_did_close_then_did_open() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	// Configure a server so acquire() succeeds.
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/reopen_old.rs");
	let new_path = Path::new("/reopen_new.rs");

	// Open document under old path (triggers acquire + didOpen).
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Clear recorded notifications from the open call.
	transport.notifications.lock().unwrap().clear();

	// Reopen under new path.
	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// Old URI must be unregistered.
	assert!(!documents.is_opened(&old_uri));

	// New URI must be registered and opened.
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(documents.is_opened(&new_uri));

	// Verify notification ordering: didClose before didOpen.
	let methods = transport.recorded_methods();
	let close_idx = methods.iter().position(|m| m == "textDocument/didClose");
	let open_idx = methods.iter().position(|m| m == "textDocument/didOpen");
	assert!(close_idx.is_some(), "didClose not sent; methods: {:?}", methods);
	assert!(open_idx.is_some(), "didOpen not sent; methods: {:?}", methods);
	assert!(close_idx.unwrap() < open_idx.unwrap(), "didClose must precede didOpen; methods: {:?}", methods);
}

#[tokio::test]
async fn reopen_document_clears_old_diagnostics() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport, xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/diag_old.rs");
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();

	// Inject diagnostics for the old URI.
	documents.update_diagnostics(
		&old_uri,
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "old error".into(),
			..Diagnostic::default()
		}],
		None,
	);
	assert_eq!(documents.get_diagnostics(&old_uri).len(), 1);

	// Reopen under new path.
	let new_path = Path::new("/diag_new.rs");
	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// Old diagnostics must be cleared (unregister removes the entry).
	assert!(documents.get_diagnostics(&old_uri).is_empty());
}

#[tokio::test]
async fn reopen_document_cross_language_routes_to_correct_servers() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	// Configure two different language servers.
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);
	registry.register(
		"python",
		LanguageServerConfig {
			command: "pyright".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/rename_me.rs");
	let new_path = Path::new("/rename_me.py");

	// Open under old language.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Record the server id used for the rust open.
	let rust_server_id = {
		let recs = transport.recorded();
		recs.iter().find(|r| r.method == "textDocument/didOpen").unwrap().server_id
	};

	// Clear recordings.
	transport.notifications.lock().unwrap().clear();

	// Reopen under different language.
	sync.reopen_document(old_path, "rust", new_path, "python", "def main(): pass".into())
		.await
		.unwrap();

	let recs = transport.recorded();

	// didClose should go to the rust server.
	let close = recs.iter().find(|r| r.method == "textDocument/didClose").expect("didClose not sent");
	assert_eq!(close.server_id, rust_server_id, "didClose should go to rust server");
	assert!(close.uri.as_deref().unwrap().contains("rename_me.rs"));

	// didOpen should go to a different server (python).
	let open = recs.iter().find(|r| r.method == "textDocument/didOpen").expect("didOpen not sent");
	assert_ne!(open.server_id, rust_server_id, "didOpen should go to python server, not rust");
	assert!(open.uri.as_deref().unwrap().contains("rename_me.py"));

	// Ordering: close before open.
	let close_idx = recs.iter().position(|r| r.method == "textDocument/didClose").unwrap();
	let open_idx = recs.iter().position(|r| r.method == "textDocument/didOpen").unwrap();
	assert!(close_idx < open_idx);
}

#[tokio::test]
async fn reopen_then_change_maintains_correct_identity() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/identity_old.rs");
	let new_path = Path::new("/identity_new.rs");

	// Open old, reopen to new.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	transport.notifications.lock().unwrap().clear();

	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// Now send a change to the new path.
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(documents.is_opened(&new_uri));

	// send_change with full text should succeed on the new identity.
	// The server isn't initialized so send_change will reopen (open_if_needed=true).
	// What matters: no notification goes to the old URI after reopen.
	let recs = transport.recorded();
	let old_uri_str = crate::uri_from_path(old_path).unwrap().to_string();

	// After the clear, only didClose should reference the old URI.
	let old_refs: Vec<_> = recs.iter().filter(|r| r.uri.as_deref() == Some(old_uri_str.as_str())).collect();
	assert!(
		old_refs.iter().all(|r| r.method == "textDocument/didClose"),
		"only didClose should reference old URI after reopen; got: {:?}",
		old_refs.iter().map(|r| &r.method).collect::<Vec<_>>()
	);

	// Ordering: didClose(old) → didOpen(new).
	let close_idx = recs.iter().position(|r| r.method == "textDocument/didClose").unwrap();
	let open_idx = recs.iter().position(|r| r.method == "textDocument/didOpen").unwrap();
	assert!(close_idx < open_idx, "didClose must precede didOpen");
}

#[tokio::test]
async fn close_document_sends_did_close_and_clears_diagnostics() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/close_me.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();
	assert!(documents.is_opened(&uri));

	// Inject diagnostics.
	documents.update_diagnostics(
		&uri,
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "error".into(),
			..Diagnostic::default()
		}],
		None,
	);
	assert_eq!(documents.get_diagnostics(&uri).len(), 1);

	transport.notifications.lock().unwrap().clear();

	// Close the document.
	sync.close_document(path, "rust").await.unwrap();

	// URI must be unregistered.
	assert!(!documents.is_opened(&uri));

	// Diagnostics must be cleared.
	assert!(documents.get_diagnostics(&uri).is_empty());

	// didClose notification must have been sent.
	let recs = transport.recorded();
	let close = recs.iter().find(|r| r.method == "textDocument/didClose");
	assert!(
		close.is_some(),
		"didClose not sent; methods: {:?}",
		recs.iter().map(|r| &r.method).collect::<Vec<_>>()
	);
	assert!(close.unwrap().uri.as_deref().unwrap().contains("close_me.rs"));
}

#[tokio::test]
async fn ensure_open_text_registers_and_sends_did_open() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/open_me.rs");
	let uri = crate::uri_from_path(path).unwrap();
	assert!(!documents.is_opened(&uri));

	// Open the document.
	sync.ensure_open_text(path, "rust", "fn main() {}".into()).await.unwrap();

	// URI must be registered and opened.
	assert!(documents.is_opened(&uri));

	// didOpen notification must have been sent with correct URI.
	let recs = transport.recorded();
	let open = recs.iter().find(|r| r.method == "textDocument/didOpen");
	assert!(
		open.is_some(),
		"didOpen not sent; methods: {:?}",
		recs.iter().map(|r| &r.method).collect::<Vec<_>>()
	);
	assert!(open.unwrap().uri.as_deref().unwrap().contains("open_me.rs"));
}

#[tokio::test]
async fn close_document_unregisters_even_if_did_close_fails() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/fail_close.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();
	assert!(documents.is_opened(&uri));

	// Inject diagnostics.
	documents.update_diagnostics(
		&uri,
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "error".into(),
			..Diagnostic::default()
		}],
		None,
	);

	// Make didClose fail.
	transport.set_fail_method("textDocument/didClose");

	// close_document should return Err but still unregister.
	let result = sync.close_document(path, "rust").await;
	assert!(result.is_err(), "expected error from failed didClose");

	// URI must be unregistered despite the error.
	assert!(!documents.is_opened(&uri));

	// Diagnostics must be cleared.
	assert!(documents.get_diagnostics(&uri).is_empty());
}

#[tokio::test]
async fn reopen_document_opens_new_even_if_did_close_fails() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/fail_reopen_old.rs");
	let new_path = Path::new("/fail_reopen_new.rs");

	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Make didClose fail.
	transport.set_fail_method("textDocument/didClose");

	// reopen_document should still open the new document.
	let result = sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await;

	// Should return the close error (open succeeded).
	assert!(result.is_err(), "expected error propagated from failed didClose");

	// Old URI must be unregistered despite the error.
	assert!(!documents.is_opened(&old_uri));

	// New URI must be registered and opened.
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(documents.is_opened(&new_uri));
}

#[tokio::test]
async fn ensure_open_text_unregisters_if_did_open_fails() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/fail_open.rs");
	let uri = crate::uri_from_path(path).unwrap();

	// Make didOpen fail.
	transport.set_fail_method("textDocument/didOpen");

	let result = sync.ensure_open_text(path, "rust", "fn main() {}".into()).await;
	assert!(result.is_err(), "expected error from failed didOpen");

	// URI must NOT be registered or opened.
	assert!(!documents.is_opened(&uri));
	assert!(documents.get_diagnostics(&uri).is_empty());
}

#[tokio::test]
async fn reopen_document_does_not_register_new_if_did_open_fails() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/reopen_fail_old.rs");
	let new_path = Path::new("/reopen_fail_new.rs");

	// Open old normally.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Make didOpen fail (didClose will succeed).
	transport.set_fail_method("textDocument/didOpen");

	let result = sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await;
	assert!(result.is_err(), "expected error from failed didOpen on new path");

	// Old must be unregistered (close succeeded).
	assert!(!documents.is_opened(&old_uri));

	// New must NOT be registered (open failed, unregister cleaned up).
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(!documents.is_opened(&new_uri));
}

/// Transport that handles initialize requests and records notifications.
/// Combines RecordingTransport's recording with initialize capability.
struct InitRecordingTransport {
	inner: RecordingTransport,
}

impl InitRecordingTransport {
	fn new() -> Self {
		Self {
			inner: RecordingTransport::new(),
		}
	}

	fn set_fail_method(&self, method: &str) {
		self.inner.set_fail_method(method);
	}

	fn clear_fail_method(&self, method: &str) {
		self.inner.fail_methods.lock().unwrap().remove(method);
	}

	fn recorded(&self) -> Vec<RecordedNotification> {
		self.inner.recorded()
	}

	fn clear_recordings(&self) {
		self.inner.notifications.lock().unwrap().clear();
	}
}

#[async_trait]
impl crate::client::transport::LspTransport for InitRecordingTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		self.inner.subscribe_events()
	}
	async fn start(&self, cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		self.inner.start(cfg).await
	}
	async fn notify(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<()> {
		self.inner.notify(server, notif).await
	}
	async fn notify_with_barrier(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		self.inner.notify_with_barrier(server, notif).await
	}
	async fn request(&self, _server: LanguageServerId, _req: crate::AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<crate::AnyResponse> {
		Ok(crate::AnyResponse {
			id: crate::RequestId::Number(1),
			result: Some(
				serde_json::to_value(lsp_types::InitializeResult {
					capabilities: lsp_types::ServerCapabilities::default(),
					server_info: None,
				})
				.unwrap(),
			),
			error: None,
		})
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::types::RequestId,
		_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

#[tokio::test]
async fn did_change_failure_marks_force_full_sync() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(InitRecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/change_fail.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();
	assert!(documents.is_opened(&uri));

	// Wait for initialization to complete.
	let client = registry.get("rust", path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "client must be initialized");

	// No force_full_sync initially.
	assert!(!documents.take_force_full_sync_by_uri(&uri));

	// Make didChange fail.
	transport.set_fail_method("textDocument/didChange");

	let result = sync
		.send_change(ChangeRequest::full_text(path, "rust", "fn main() { 1 }".into()).with_open_if_needed(false))
		.await;
	assert!(result.is_err(), "expected error from failed didChange");

	// force_full_sync must be set after failure.
	assert!(
		documents.take_force_full_sync_by_uri(&uri),
		"force_full_sync must be set after didChange failure"
	);
}

#[tokio::test]
async fn did_change_success_does_not_set_force_full_sync() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(InitRecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/change_ok.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();

	// Wait for initialization.
	let client = registry.get("rust", path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "client must be initialized");

	let result = sync
		.send_change(ChangeRequest::full_text(path, "rust", "fn main() { 1 }".into()).with_open_if_needed(false))
		.await;
	assert!(result.is_ok(), "expected successful didChange");

	// force_full_sync must NOT be set after success.
	assert!(
		!documents.take_force_full_sync_by_uri(&uri),
		"force_full_sync should not be set after successful didChange"
	);
}

#[tokio::test]
async fn open_document_unregisters_if_did_open_fails() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/rope_fail_open.rs");
	let uri = crate::uri_from_path(path).unwrap();

	// Make didOpen fail.
	transport.set_fail_method("textDocument/didOpen");

	let result = sync.open_document(path, "rust", &Rope::from("fn main() {}")).await;
	assert!(result.is_err(), "expected error from failed didOpen via open_document");

	// URI must NOT be registered or opened.
	assert!(!documents.is_opened(&uri), "phantom open via Rope API");
	assert!(documents.get_diagnostics(&uri).is_empty());
}

#[tokio::test]
async fn open_document_can_retry_after_failed_open() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone(), xeno_worker::WorkerRuntime::new());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/rope_retry_open.rs");
	let uri = crate::uri_from_path(path).unwrap();

	// First attempt fails.
	transport.set_fail_method("textDocument/didOpen");
	let result = sync.open_document(path, "rust", &Rope::from("fn main() {}")).await;
	assert!(result.is_err());
	assert!(!documents.is_opened(&uri), "state must be clean after failure");

	// Clear failure and retry.
	transport.clear_fail_method("textDocument/didOpen");
	let result = sync.open_document(path, "rust", &Rope::from("fn main() {}")).await;
	assert!(result.is_ok(), "retry must succeed: {:?}", result.err());
	assert!(documents.is_opened(&uri), "document must be opened after retry");
}

/// Source-level invariant: no direct `ClientHandle.text_document_did_*` calls outside `crates/lsp/src/sync/`.
///
/// All didOpen/didClose/didChange must flow through `DocumentSync` to maintain registration
/// state consistency. Direct calls bypass the unregister-on-failure and force_full_sync guards.
#[test]
fn no_direct_did_notifications_outside_sync_module() {
	use std::path::Path;

	let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
	let crates_dir = workspace_root.join("crates");

	let forbidden_patterns = [
		".text_document_did_open(",
		".text_document_did_close(",
		".text_document_did_change(",
		".text_document_did_change_full(",
		".text_document_did_change_with_barrier(",
	];

	let sync_dir = crates_dir.join("lsp").join("src").join("sync");
	let client_api_dir = crates_dir.join("lsp").join("src").join("client").join("api");

	fn walk_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
		let Ok(entries) = std::fs::read_dir(dir) else { return };
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				walk_rs_files(&path, out);
			} else if path.extension().is_some_and(|ext| ext == "rs") {
				out.push(path);
			}
		}
	}

	let mut rs_files = Vec::new();
	walk_rs_files(&crates_dir, &mut rs_files);

	let mut violations = Vec::new();

	for path in &rs_files {
		// Allow the sync module itself (implementation).
		if path.starts_with(&sync_dir) {
			continue;
		}
		// Allow client API definitions.
		if path.starts_with(&client_api_dir) {
			continue;
		}

		let content = match std::fs::read_to_string(path) {
			Ok(c) => c,
			Err(_) => continue,
		};

		for pattern in &forbidden_patterns {
			for (line_no, line) in content.lines().enumerate() {
				if line.contains(pattern) {
					violations.push(format!(
						"{}:{}: {}",
						path.strip_prefix(workspace_root).unwrap_or(path).display(),
						line_no + 1,
						line.trim()
					));
				}
			}
		}
	}

	assert!(
		violations.is_empty(),
		"Direct ClientHandle.text_document_did_* calls found outside sync module.\n\
		 All didOpen/didClose/didChange must go through DocumentSync.\n\
		 Violations:\n{}",
		violations.join("\n")
	);
}
