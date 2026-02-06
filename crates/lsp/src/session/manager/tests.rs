use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};

use super::*;
use crate::client::LanguageServerId;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, ResponseError};

struct StubTransport;

#[async_trait]
impl LspTransport for StubTransport {
	fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
		let (_, rx) = mpsc::unbounded_channel();
		rx
	}

	async fn start(
		&self,
		_cfg: crate::client::ServerConfig,
	) -> crate::Result<crate::client::transport::StartedServer> {
		Err(crate::Error::Protocol("StubTransport".into()))
	}

	async fn notify(
		&self,
		_server: LanguageServerId,
		_notif: AnyNotification,
	) -> crate::Result<()> {
		Ok(())
	}

	async fn notify_with_barrier(
		&self,
		_server: LanguageServerId,
		_notif: AnyNotification,
	) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}

	async fn request(
		&self,
		_server: LanguageServerId,
		_req: AnyRequest,
		_timeout: Option<std::time::Duration>,
	) -> crate::Result<AnyResponse> {
		Err(crate::Error::Protocol("StubTransport".into()))
	}

	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::types::RequestId,
		_resp: Result<JsonValue, ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}

	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

#[test]
fn test_lsp_manager_creation() {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport);
	let manager = LspManager::new(transport);
	assert_eq!(manager.diagnostics_version(), 0);
}

#[test]
fn test_router_no_runtime() {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport);
	let manager = LspManager::new(transport);

	match manager.spawn_router() {
		Err(SpawnRouterError::NoRuntime) => {}
		other => panic!("expected NoRuntime, got: {:?}", other),
	}
}

#[test]
fn test_router_single_spawn() {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport);
	let manager = LspManager::new(transport);

	let rt = tokio::runtime::Runtime::new().unwrap();
	let _guard = rt.enter();

	assert!(manager.spawn_router().is_ok());

	match manager.spawn_router() {
		Err(SpawnRouterError::AlreadyStarted) => {}
		other => panic!("expected AlreadyStarted, got: {:?}", other),
	}
}
