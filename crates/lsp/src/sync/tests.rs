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
	fn events(&self) -> mpsc::UnboundedReceiver<crate::client::transport::TransportEvent> {
		let (_, rx) = mpsc::unbounded_channel();
		rx
	}
	async fn start(
		&self,
		_cfg: crate::client::ServerConfig,
	) -> crate::Result<crate::client::transport::StartedServer> {
		Ok(crate::client::transport::StartedServer {
			id: LanguageServerId::new(1, 0),
		})
	}
	async fn notify(
		&self,
		_server: LanguageServerId,
		_notif: crate::AnyNotification,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn notify_with_barrier(
		&self,
		_server: LanguageServerId,
		_notif: crate::AnyNotification,
	) -> crate::Result<oneshot::Receiver<()>> {
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(());
		Ok(rx)
	}
	async fn request(
		&self,
		_server: LanguageServerId,
		_req: crate::AnyRequest,
		_timeout: Option<std::time::Duration>,
	) -> crate::Result<crate::AnyResponse> {
		Err(crate::Error::Protocol("SimpleStubTransport".into()))
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
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
	let registry = Arc::new(Registry::new(transport));
	let documents = Arc::new(DocumentStateManager::new());
	let sync = DocumentSync::with_registry(registry, documents);

	assert_eq!(sync.total_error_count(), 0);
	assert_eq!(sync.total_warning_count(), 0);
}

#[tokio::test]
async fn test_document_sync_create() {
	let transport = Arc::new(SimpleStubTransport);
	let (sync, _registry, _documents, _receiver) = DocumentSync::create(transport);

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
		fn events(&self) -> mpsc::UnboundedReceiver<crate::client::transport::TransportEvent> {
			let (_, rx) = mpsc::unbounded_channel();
			rx
		}
		async fn start(
			&self,
			_cfg: crate::client::ServerConfig,
		) -> crate::Result<crate::client::transport::StartedServer> {
			Ok(crate::client::transport::StartedServer {
				id: LanguageServerId::new(1, 0),
			})
		}
		async fn notify(
			&self,
			_server: LanguageServerId,
			_notif: crate::AnyNotification,
		) -> crate::Result<()> {
			Ok(())
		}
		async fn notify_with_barrier(
			&self,
			_server: LanguageServerId,
			_notif: crate::AnyNotification,
		) -> crate::Result<oneshot::Receiver<()>> {
			let (tx, rx) = oneshot::channel();
			let _ = tx.send(());
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
			_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
		) -> crate::Result<()> {
			Ok(())
		}
		async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
			Ok(())
		}
	}

	let transport = Arc::new(InitStubTransport);
	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport);

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

	// get_or_start will spawn initialize in background
	let client = registry.get("rust", path).unwrap();

	// Ensure it's NOT initialized yet (background task might not have run)
	if !client.is_initialized() {
		let result = sync.notify_change_full(path, "rust", &content).await;
		assert!(matches!(result, Err(crate::Error::NotReady)));
	}
}
