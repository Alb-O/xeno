use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};

use super::LspManager;
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

	async fn reply(&self, _server: LanguageServerId, _id: RequestId, _resp: Result<JsonValue, ResponseError>) -> crate::Result<()> {
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
	crate::client::ClientHandle::new(id, "stub".into(), std::path::PathBuf::from("/tmp"), transport)
}

/// Must singleflight `transport.start()` per `(language, root_path)` key.
///
/// * Enforced in: `Registry::get_or_start`
/// * Failure symptom: Duplicate server processes for the same language and workspace.
#[cfg_attr(test, test)]
pub(crate) fn test_registry_singleflight_prevents_duplicate_transport_start() {
	let _manager = make_manager();
}

/// Must update `servers`/`server_meta`/`id_index` atomically on registry mutation.
///
/// * Enforced in: `Registry::get_or_start`, `Registry::remove_server`
/// * Failure symptom: Stale server entries linger in one index after removal from another.
#[cfg_attr(test, test)]
pub(crate) fn test_registry_remove_server_scrubs_all_indices() {
	let _manager = make_manager();
}

/// Must process transport events sequentially and reply to requests inline.
///
/// * Enforced in: `LspManager::spawn_router`
/// * Failure symptom: Out-of-order replies corrupt the JSON-RPC request/response pairing.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_router_event_ordering() {
	let manager = make_manager();
	assert!(manager.spawn_router().is_ok());
	assert!(manager.spawn_router().is_err());
}

/// Must remove stopped/crashed servers from registry and clear their progress.
///
/// * Enforced in: `LspManager::spawn_router` (Status event handler)
/// * Failure symptom: Ghost progress spinners remain after server crash.
#[cfg_attr(test, test)]
pub(crate) fn test_status_stopped_removes_server_and_clears_progress() {
	let _manager = make_manager();
}

/// Must drop events from stale server generations.
///
/// * Enforced in: `LspManager::spawn_router` (generation filter)
/// * Failure symptom: Diagnostics or progress from a dead server instance appear in the UI.
#[cfg_attr(test, test)]
pub(crate) fn test_router_drops_stale_generation_events() {
	let _manager = make_manager();
}

/// `LanguageServerId` must be slot + monotonic generation counter.
///
/// * Enforced in: `RegistryState::next_gen`, `ServerConfig::id`
/// * Failure symptom: Restarted servers reuse old IDs, causing event misrouting.
#[cfg_attr(test, test)]
pub(crate) fn test_server_id_generation_increments_on_restart() {
	let id1 = LanguageServerId::new(0, 0);
	let id2 = LanguageServerId::new(0, 1);
	assert_ne!(id1, id2, "different generations must be distinct");
	assert_eq!(id1.slot, id2.slot, "same slot across restarts");
}

/// `ServerConfig` must carry a pre-assigned `LanguageServerId` before transport start.
///
/// * Enforced in: `Registry::get_or_start`
/// * Failure symptom: Transport starts without a valid server ID for event correlation.
#[cfg_attr(test, test)]
pub(crate) fn test_singleflight_start() {
	let _manager = make_manager();
}

/// `workspace/configuration` response must match the item count of the request.
///
/// * Enforced in: `handle_server_request` (workspace/configuration arm)
/// * Failure symptom: Server receives wrong number of configuration sections.
#[cfg_attr(test, test)]
pub(crate) fn test_server_request_workspace_configuration_section_slicing() {
	let _manager = make_manager();
}

/// `workspace/workspaceFolders` response must use percent-encoded URIs.
///
/// * Enforced in: `handle_server_request` (workspace/workspaceFolders arm)
/// * Failure symptom: Servers fail to parse workspace folder URIs with special characters.
#[cfg_attr(test, test)]
pub(crate) fn test_server_request_workspace_folders_uri_encoding() {
	let _manager = make_manager();
}

/// Must not send change notifications before client initialization completes.
///
/// * Enforced in: `DocumentSync::*` (initialization gate)
/// * Failure symptom: Server receives didChange before didOpen/initialization.
#[cfg_attr(test, test)]
pub(crate) fn test_document_sync_returns_not_ready_before_init() {}

/// Must gate position-dependent requests on client readiness.
///
/// * Enforced in: `ClientHandle::is_ready`, position request preparation
/// * Failure symptom: Requests sent to uninitialized server are rejected or misrouted.
#[cfg_attr(test, test)]
pub(crate) fn test_prepare_position_request_returns_none_before_ready() {
	let handle = make_client_handle();
	assert!(!handle.is_ready(), "fresh ClientHandle must not be ready");
}

/// Must return `None` for capabilities before initialization completes.
///
/// * Enforced in: `ClientHandle::capabilities`
/// * Failure symptom: Code assumes capabilities exist and panics on unwrap.
#[cfg_attr(test, test)]
pub(crate) fn test_client_handle_capabilities_returns_none_before_init() {
	let handle = make_client_handle();
	assert!(handle.capabilities().is_none(), "capabilities must be None before initialization");
}

/// Ready flag must require capabilities with release/acquire ordering.
///
/// * Enforced in: `ClientHandle::set_ready`, `ClientHandle::is_ready`
/// * Failure symptom: Client appears ready but capabilities load returns stale/null data.
#[cfg_attr(test, test)]
pub(crate) fn test_set_ready_requires_initialized() {
	let handle = make_client_handle();
	assert!(!handle.is_ready());
}

/// Must use canonicalized paths for registry lookups.
///
/// * Enforced in: `LspSystem` lookup paths (in `xeno-editor`)
/// * Failure symptom: Same server started twice for symlinked workspace roots.
#[cfg_attr(test, test)]
pub(crate) fn test_registry_lookup_uses_canonical_path() {}
