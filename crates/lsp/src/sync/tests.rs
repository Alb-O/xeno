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
