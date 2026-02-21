use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};

use super::*;
use crate::client::LanguageServerId;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, ResponseError};

struct StubTransport {
	events_rx: Mutex<Option<mpsc::UnboundedReceiver<TransportEvent>>>,
}

impl StubTransport {
	fn new() -> Self {
		let (_tx, rx) = mpsc::unbounded_channel();
		Self {
			events_rx: Mutex::new(Some(rx)),
		}
	}
}

#[async_trait]
impl LspTransport for StubTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<TransportEvent>> {
		self.events_rx
			.lock()
			.take()
			.ok_or_else(|| crate::Error::Protocol("events already subscribed".into()))
	}

	async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		Err(crate::Error::Protocol("StubTransport".into()))
	}

	async fn notify(&self, _server: LanguageServerId, _notif: AnyNotification) -> crate::Result<()> {
		Ok(())
	}

	async fn notify_with_barrier(&self, _server: LanguageServerId, _notif: AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}

	async fn request(&self, _server: LanguageServerId, _req: AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<AnyResponse> {
		Err(crate::Error::Protocol("StubTransport".into()))
	}

	async fn reply(&self, _server: LanguageServerId, _id: crate::types::RequestId, _resp: Result<JsonValue, ResponseError>) -> crate::Result<()> {
		Ok(())
	}

	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

#[test]
fn test_lsp_session_creation() {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport::new());
	let (session, runtime) = LspSession::new(transport);
	assert_eq!(session.diagnostics_version(), 0);
	assert!(!runtime.is_started());
}

#[test]
fn test_runtime_start_requires_tokio_runtime() {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport::new());
	let (_session, runtime) = LspSession::new(transport);

	match runtime.start() {
		Err(RuntimeStartError::NoRuntime) => {}
		other => panic!("expected NoRuntime, got: {other:?}"),
	}
}

#[test]
fn test_runtime_single_start() {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport::new());
	let (_session, runtime) = LspSession::new(transport);

	let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
	let _guard = rt.enter();

	assert!(runtime.start().is_ok());

	match runtime.start() {
		Err(RuntimeStartError::AlreadyStarted) => {}
		other => panic!("expected AlreadyStarted, got: {other:?}"),
	}
}
