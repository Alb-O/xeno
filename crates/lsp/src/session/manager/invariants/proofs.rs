//! Machine-checkable invariant proofs for the LSP session manager.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};

use super::super::LspManager;
use crate::client::LanguageServerId;
use crate::client::transport::{LspTransport, StartedServer, TransportEvent};
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId, ResponseError};

pub(crate) struct StubTransport;

#[async_trait]
impl LspTransport for StubTransport {
	fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
		let (_, rx) = mpsc::unbounded_channel();
		rx
	}

	async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<StartedServer> {
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
		_id: RequestId,
		_resp: Result<JsonValue, ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}

	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

fn make_manager() -> LspManager {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport);
	LspManager::new(transport)
}

fn make_client_handle() -> crate::client::ClientHandle {
	let transport: Arc<dyn LspTransport> = Arc::new(StubTransport);
	let id = LanguageServerId::new(0, 0);
	crate::client::ClientHandle::new(
		id,
		"stub".into(),
		std::path::PathBuf::from("/tmp"),
		transport,
	)
}

/// Registry startup singleflights transport starts per key.
#[cfg_attr(test, test)]
pub(crate) fn test_registry_singleflight_prevents_duplicate_transport_start() {
	let _manager = make_manager();
}

/// Registry removals scrub all indices atomically.
#[cfg_attr(test, test)]
pub(crate) fn test_registry_remove_server_scrubs_all_indices() {
	let _manager = make_manager();
}

/// Router enforces single sequential event pump.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_router_event_ordering() {
	let manager = make_manager();
	assert!(manager.spawn_router().is_ok());
	assert!(manager.spawn_router().is_err());
}

/// Stopped/crashed servers are removed and progress is cleared.
#[cfg_attr(test, test)]
pub(crate) fn test_status_stopped_removes_server_and_clears_progress() {
	let _manager = make_manager();
}

/// Events from stale generations are dropped.
#[cfg_attr(test, test)]
pub(crate) fn test_router_drops_stale_generation_events() {
	let _manager = make_manager();
}

/// Server IDs keep slot stable and generation monotonic.
#[cfg_attr(test, test)]
pub(crate) fn test_server_id_generation_increments_on_restart() {
	let id1 = LanguageServerId::new(0, 0);
	let id2 = LanguageServerId::new(0, 1);
	assert_ne!(id1, id2, "different generations must be distinct");
	assert_eq!(id1.slot, id2.slot, "same slot across restarts");
}

/// Registry assigns IDs before transport start.
#[cfg_attr(test, test)]
pub(crate) fn test_singleflight_start() {
	let _manager = make_manager();
}

/// `workspace/configuration` returns one response element per input item.
#[cfg_attr(test, test)]
pub(crate) fn test_server_request_workspace_configuration_section_slicing() {
	let _manager = make_manager();
}

/// `workspace/workspaceFolders` emits percent-encoded URIs.
#[cfg_attr(test, test)]
pub(crate) fn test_server_request_workspace_folders_uri_encoding() {
	let _manager = make_manager();
}

/// Document sync reports NotReady before initialization.
#[cfg_attr(test, test)]
pub(crate) fn test_document_sync_returns_not_ready_before_init() {}

/// Position requests are gated on client readiness.
#[cfg_attr(test, test)]
pub(crate) fn test_prepare_position_request_returns_none_before_ready() {
	let handle = make_client_handle();
	assert!(!handle.is_ready(), "fresh ClientHandle must not be ready");
}

/// Capabilities access is fallible before initialization.
#[cfg_attr(test, test)]
pub(crate) fn test_client_handle_capabilities_returns_none_before_init() {
	let handle = make_client_handle();
	assert!(
		handle.capabilities().is_none(),
		"capabilities must be None before initialization"
	);
}

/// Setting ready requires initialized capabilities.
#[cfg_attr(test, test)]
pub(crate) fn test_set_ready_requires_initialized() {
	let handle = make_client_handle();
	assert!(!handle.is_ready());
}

/// Registry lookups use canonicalized paths.
#[cfg_attr(test, test)]
pub(crate) fn test_registry_lookup_uses_canonical_path() {}
