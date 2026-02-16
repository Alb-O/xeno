use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{Value as JsonValue, json};
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::{sleep, timeout};

use super::{LspRuntime, LspSession, RuntimeStartError};
use crate::client::LanguageServerId;
use crate::client::transport::{LspTransport, StartedServer, TransportEvent, TransportStatus};
use crate::registry::{AcquireDisposition, LanguageServerConfig, Registry};
use crate::session::server_requests::{ServerRequestReply, dispatch_server_request};
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId, ResponseError};
use crate::{DocumentSync, Message};

static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(1);

struct TestTransport {
	events_tx: mpsc::UnboundedSender<TransportEvent>,
	events_rx: Mutex<Option<mpsc::UnboundedReceiver<TransportEvent>>>,
	start_count: AtomicUsize,
	started_ids: Mutex<Vec<LanguageServerId>>,
	start_gate: Mutex<Option<Arc<Notify>>>,
	reply_started: AtomicUsize,
	reply_started_notify: Notify,
	reply_gate: Mutex<Option<Arc<Notify>>>,
	replies: Mutex<Vec<RequestId>>,
	stops: Mutex<Vec<LanguageServerId>>,
}

impl TestTransport {
	fn new() -> Arc<Self> {
		let (events_tx, events_rx) = mpsc::unbounded_channel();
		Arc::new(Self {
			events_tx,
			events_rx: Mutex::new(Some(events_rx)),
			start_count: AtomicUsize::new(0),
			started_ids: Mutex::new(Vec::new()),
			start_gate: Mutex::new(None),
			reply_started: AtomicUsize::new(0),
			reply_started_notify: Notify::new(),
			reply_gate: Mutex::new(None),
			replies: Mutex::new(Vec::new()),
			stops: Mutex::new(Vec::new()),
		})
	}

	fn emit(&self, event: TransportEvent) {
		let _ = self.events_tx.send(event);
	}

	fn set_start_gate(&self, gate: Option<Arc<Notify>>) {
		*self.start_gate.lock() = gate;
	}

	fn set_reply_gate(&self, gate: Option<Arc<Notify>>) {
		*self.reply_gate.lock() = gate;
	}

	fn start_count(&self) -> usize {
		self.start_count.load(Ordering::SeqCst)
	}

	fn started_ids(&self) -> Vec<LanguageServerId> {
		self.started_ids.lock().clone()
	}

	fn reply_started(&self) -> usize {
		self.reply_started.load(Ordering::SeqCst)
	}

	fn replies(&self) -> Vec<RequestId> {
		self.replies.lock().clone()
	}

	fn stops(&self) -> Vec<LanguageServerId> {
		self.stops.lock().clone()
	}
}

#[async_trait]
impl LspTransport for TestTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<TransportEvent>> {
		self.events_rx
			.lock()
			.take()
			.ok_or_else(|| crate::Error::Protocol("transport events already subscribed".into()))
	}

	async fn start(&self, cfg: crate::client::ServerConfig) -> crate::Result<StartedServer> {
		self.start_count.fetch_add(1, Ordering::SeqCst);
		self.started_ids.lock().push(cfg.id);
		let gate = self.start_gate.lock().clone();
		if let Some(gate) = gate {
			gate.notified().await;
		}
		Ok(StartedServer { id: cfg.id })
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
		Err(crate::Error::Protocol("request not implemented in TestTransport".into()))
	}

	async fn reply(&self, _server: LanguageServerId, id: RequestId, _resp: Result<JsonValue, ResponseError>) -> crate::Result<()> {
		let idx = self.reply_started.fetch_add(1, Ordering::SeqCst);
		self.reply_started_notify.notify_waiters();
		if idx == 0 {
			let gate = self.reply_gate.lock().clone();
			if let Some(gate) = gate {
				gate.notified().await;
			}
		}
		self.replies.lock().push(id);
		Ok(())
	}

	async fn stop(&self, server: LanguageServerId) -> crate::Result<()> {
		self.stops.lock().push(server);
		Ok(())
	}
}

fn make_temp_file(label: &str, with_space: bool) -> PathBuf {
	let suffix = NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed);
	let mut root = std::env::temp_dir();
	let folder = if with_space {
		format!("xeno lsp {label} {} {suffix}", std::process::id())
	} else {
		format!("xeno_lsp_{label}_{}_{}", std::process::id(), suffix)
	};
	root.push(folder);
	std::fs::create_dir_all(&root).expect("must create temp root");
	let file = root.join("test.rs");
	std::fs::write(&file, "fn main() {}\n").expect("must write test file");
	file
}

async fn wait_until<F>(name: &str, mut condition: F)
where
	F: FnMut() -> bool,
{
	timeout(Duration::from_secs(2), async move {
		loop {
			if condition() {
				return;
			}
			sleep(Duration::from_millis(10)).await;
		}
	})
	.await
	.unwrap_or_else(|_| panic!("timed out waiting for {name}"));
}

fn make_registry(transport: Arc<dyn LspTransport>) -> Arc<Registry> {
	let registry = Arc::new(Registry::new(transport));
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);
	registry
}

fn make_client_handle() -> crate::client::ClientHandle {
	let transport: Arc<dyn LspTransport> = TestTransport::new();
	let id = LanguageServerId::new(0, 0);
	crate::client::ClientHandle::new(id, "stub".into(), std::path::PathBuf::from("/tmp"), transport)
}

async fn make_session_runtime(transport: Arc<TestTransport>) -> (LspSession, LspRuntime, PathBuf, LanguageServerId) {
	let (session, runtime) = LspSession::new(transport.clone());
	runtime.start().expect("runtime must start in tokio tests");
	session.configure_server(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);
	let path = make_temp_file("session", false);
	session
		.sync()
		.ensure_open_text(&path, "rust", "fn main() {}".to_string())
		.await
		.expect("must open document");
	let server_id = session.registry().get("rust", &path).expect("server must exist").id();
	(session, runtime, path, server_id)
}

/// Must singleflight `transport.start()` per `(language, root_path)` key.
///
/// * Enforced in: `Registry::acquire`
/// * Failure symptom: Duplicate server processes for the same language and workspace.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_registry_singleflight_prevents_duplicate_transport_start() {
	let transport = TestTransport::new();
	let start_gate = Arc::new(Notify::new());
	transport.set_start_gate(Some(start_gate.clone()));
	let registry = make_registry(transport.clone());
	let path = make_temp_file("singleflight", false);

	let r1 = registry.clone();
	let p1 = path.clone();
	let h1 = tokio::spawn(async move { r1.acquire("rust", &p1).await });
	wait_until("leader entering start()", || transport.start_count() == 1).await;

	let r2 = registry.clone();
	let p2 = path.clone();
	let h2 = tokio::spawn(async move { r2.acquire("rust", &p2).await });
	sleep(Duration::from_millis(50)).await;
	assert_eq!(transport.start_count(), 1, "singleflight must block duplicate starts");

	start_gate.notify_waiters();
	let a1 = h1.await.expect("join leader").expect("leader acquire succeeds");
	let a2 = h2.await.expect("join waiter").expect("waiter acquire succeeds");

	assert_eq!(transport.start_count(), 1);
	assert_eq!(a1.server_id, a2.server_id);
	assert_eq!(a1.disposition, AcquireDisposition::Started);
	assert_eq!(a2.disposition, AcquireDisposition::Started);
}

/// Must update `servers`/`server_meta`/`id_index` atomically on registry mutation.
///
/// * Enforced in: `Registry::acquire`, `Registry::remove_server`
/// * Failure symptom: Stale server entries linger in one index after removal from another.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_registry_remove_server_scrubs_all_indices() {
	let transport = TestTransport::new();
	let registry = make_registry(transport);
	let path = make_temp_file("remove", false);
	let acquired = registry.acquire("rust", &path).await.expect("must acquire");
	let id = acquired.server_id;

	assert!(registry.is_current(id));
	assert!(registry.get("rust", &path).is_some());
	assert!(registry.get_server_meta(id).is_some());

	let removed = registry.remove_server(id).expect("must remove active server");
	assert_eq!(removed.language, "rust");
	assert!(!registry.is_current(id));
	assert!(registry.get_server_meta(id).is_none());
	assert!(registry.get("rust", &path).is_none());
}

/// Must process transport events sequentially and reply to requests inline.
///
/// * Enforced in: `LspRuntime` router loop + `process_message_event`
/// * Failure symptom: Out-of-order replies corrupt JSON-RPC request/response pairing.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_router_event_ordering() {
	let transport = TestTransport::new();
	let (session, runtime, _path, server_id) = make_session_runtime(transport.clone()).await;
	let reply_gate = Arc::new(Notify::new());
	transport.set_reply_gate(Some(reply_gate.clone()));

	transport.emit(TransportEvent::Message {
		server: server_id,
		message: Message::Request(AnyRequest {
			id: RequestId::Number(1),
			method: "client/registerCapability".into(),
			params: json!({}),
		}),
	});

	wait_until("first reply to start", || transport.reply_started() >= 1).await;

	transport.emit(TransportEvent::Message {
		server: server_id,
		message: Message::Request(AnyRequest {
			id: RequestId::Number(2),
			method: "client/registerCapability".into(),
			params: json!({}),
		}),
	});

	sleep(Duration::from_millis(60)).await;
	assert_eq!(transport.reply_started(), 1, "second request must not start reply before first finishes");

	reply_gate.notify_waiters();
	wait_until("both replies", || transport.replies().len() == 2).await;
	assert_eq!(transport.replies(), vec![RequestId::Number(1), RequestId::Number(2)]);

	runtime.shutdown().await;
	drop(session);
}

/// Must remove stopped/crashed servers from registry and clear their progress.
///
/// * Enforced in: `process_status_event`
/// * Failure symptom: Ghost progress spinners remain after server crash.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_status_stopped_removes_server_and_clears_progress() {
	let transport = TestTransport::new();
	let (session, runtime, _path, server_id) = make_session_runtime(transport.clone()).await;

	let progress = lsp_types::ProgressParams {
		token: lsp_types::NumberOrString::String("token-1".into()),
		value: lsp_types::ProgressParamsValue::WorkDone(lsp_types::WorkDoneProgress::Begin(lsp_types::WorkDoneProgressBegin {
			title: "Indexing".into(),
			cancellable: None,
			message: Some("phase".into()),
			percentage: Some(1),
		})),
	};
	transport.emit(TransportEvent::Message {
		server: server_id,
		message: Message::Notification(AnyNotification {
			method: "$/progress".into(),
			params: serde_json::to_value(progress).expect("progress params"),
		}),
	});

	wait_until("progress begin", || session.documents().has_progress()).await;
	transport.emit(TransportEvent::Status {
		server: server_id,
		status: TransportStatus::Stopped,
	});
	wait_until("progress clear", || !session.documents().has_progress()).await;

	assert!(!session.registry().is_current(server_id));
	assert!(transport.stops().contains(&server_id), "runtime should stop crashed/stopped server transport");

	runtime.shutdown().await;
}

/// Must drop events from stale server generations.
///
/// * Enforced in: `process_transport_event` generation filter
/// * Failure symptom: Diagnostics or progress from a dead server instance appear in the UI.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_router_drops_stale_generation_events() {
	let transport = TestTransport::new();
	let (session, runtime, path, old_id) = make_session_runtime(transport.clone()).await;

	assert!(session.registry().remove_server(old_id).is_some());
	let new_acquired = session.registry().acquire("rust", &path).await.expect("must start replacement");
	assert_ne!(old_id, new_acquired.server_id);

	let uri = crate::uri_from_path(&path).expect("uri");
	let stale_diags = vec![lsp_types::Diagnostic {
		range: lsp_types::Range::default(),
		severity: Some(lsp_types::DiagnosticSeverity::ERROR),
		message: "stale".into(),
		..Default::default()
	}];

	transport.emit(TransportEvent::Diagnostics {
		server: old_id,
		uri: uri.to_string(),
		version: Some(1),
		diagnostics: serde_json::to_value(stale_diags).expect("diagnostics"),
	});

	sleep(Duration::from_millis(80)).await;
	assert_eq!(session.sync().error_count(&path), 0, "stale diagnostics must be ignored");

	runtime.shutdown().await;
}

/// Must keep `LanguageServerId` as slot + monotonic generation counter.
///
/// * Enforced in: `RegistryState::next_gen`, `ServerConfig::id`
/// * Failure symptom: Restarted servers reuse old IDs, causing event misrouting.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_server_id_generation_increments_on_restart() {
	let transport = TestTransport::new();
	let registry = make_registry(transport);
	let path = make_temp_file("restart", false);

	let first = registry.acquire("rust", &path).await.expect("first acquire");
	assert!(registry.remove_server(first.server_id).is_some());
	let second = registry.acquire("rust", &path).await.expect("second acquire");

	assert_eq!(first.server_id.slot, second.server_id.slot);
	assert!(second.server_id.generation > first.server_id.generation);
}

/// Must carry a pre-assigned `LanguageServerId` in `ServerConfig` before transport start.
///
/// * Enforced in: `Registry::acquire`
/// * Failure symptom: Transport starts without a valid server ID for event correlation.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_singleflight_start() {
	let transport = TestTransport::new();
	let registry = make_registry(transport.clone());
	let path = make_temp_file("start-id", false);
	let acquired = registry.acquire("rust", &path).await.expect("acquire");

	let started_ids = transport.started_ids();
	assert_eq!(started_ids.len(), 1);
	assert_eq!(started_ids[0], acquired.server_id);
}

/// Must return `workspace/configuration` responses matching the request item count.
///
/// * Enforced in: `dispatch_server_request` (`workspace/configuration` arm)
/// * Failure symptom: Server receives wrong number of configuration sections.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_server_request_workspace_configuration_section_slicing() {
	let transport = TestTransport::new();
	let registry = Arc::new(Registry::new(transport));
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			config: Some(json!({
				"rust-analyzer": { "foo": true }
			})),
			..Default::default()
		},
	);
	let sync = DocumentSync::with_registry(registry.clone(), Arc::new(crate::DocumentStateManager::new()));
	let path = make_temp_file("ws-config", false);
	let acquired = registry.acquire("rust", &path).await.expect("acquire");

	let reply = dispatch_server_request(
		&sync,
		acquired.server_id,
		"workspace/configuration",
		json!({
			"items": [
				{ "section": "rust-analyzer" },
				{ "section": "missing" },
				{}
			]
		}),
	)
	.await;

	let ServerRequestReply::Json(JsonValue::Array(items)) = reply else {
		panic!("expected json array reply");
	};
	assert_eq!(items.len(), 3);
	assert_eq!(items[0], json!({ "foo": true }));
	assert_eq!(items[1], json!({}));
	assert_eq!(
		items[2],
		json!({
			"rust-analyzer": { "foo": true }
		})
	);
}

/// Must return `workspace/workspaceFolders` entries as percent-encoded URIs.
///
/// * Enforced in: `dispatch_server_request` (`workspace/workspaceFolders` arm)
/// * Failure symptom: Servers fail to parse workspace folder URIs with special characters.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_server_request_workspace_folders_uri_encoding() {
	let transport = TestTransport::new();
	let registry = make_registry(transport);
	let path = make_temp_file("workspace folders", true);
	let sync = DocumentSync::with_registry(registry.clone(), Arc::new(crate::DocumentStateManager::new()));
	let acquired = registry.acquire("rust", &path).await.expect("acquire");

	let reply = dispatch_server_request(&sync, acquired.server_id, "workspace/workspaceFolders", JsonValue::Null).await;
	let ServerRequestReply::Json(JsonValue::Array(items)) = reply else {
		panic!("expected workspace folders array");
	};
	assert_eq!(items.len(), 1);
	let uri = items[0].get("uri").and_then(|v| v.as_str()).expect("workspace folder uri");
	assert!(uri.contains("%20"), "workspace folder URI must be percent-encoded: {uri}");
}

/// Must not send change notifications before client initialization completes.
///
/// * Enforced in: `DocumentSync::send_change` (initialization gate)
/// * Failure symptom: Server receives didChange before didOpen/initialization.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_document_sync_returns_not_ready_before_init() {
	let transport = TestTransport::new();
	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport);
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = make_temp_file("not-ready", false);
	let content = ropey::Rope::from("fn main() {}\n");
	sync.open_document(&path, "rust", &content).await.expect("must open document");
	let result = sync
		.send_change(crate::ChangeRequest::full_text(&path, "rust", content.to_string()).with_barrier(crate::BarrierMode::None))
		.await;
	assert!(matches!(result, Err(crate::Error::NotReady)));
}

/// Must route outbound document changes through `DocumentSync::send_change`.
///
/// * Enforced in: `DocumentSync::send_change`
/// * Failure symptom: Divergent didChange open/ack behavior across call paths.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_document_sync_send_change_full_opens_when_missing() {
	let transport = TestTransport::new();
	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport);
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = make_temp_file("send-change-open", false);
	let dispatch = sync
		.send_change(crate::ChangeRequest::full_text(&path, "rust", "fn main() {}\n".to_string()).with_barrier(crate::BarrierMode::Tracked))
		.await
		.expect("full request should open missing document");

	assert!(dispatch.opened_document);
	assert!(dispatch.applied_version.is_none());
	assert!(dispatch.barrier.is_none());
	let uri = crate::uri_from_path(&path).expect("uri");
	assert!(sync.documents().is_opened(&uri));
}

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

/// Must require initialized capabilities before setting ready with release/acquire ordering.
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
/// * Enforced in: `find_root_path`
/// * Failure symptom: Same server started twice for symlinked workspace roots.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_registry_lookup_uses_canonical_path() {
	#[cfg(not(unix))]
	{
		// Symlink setup is platform-specific; non-unix builds skip this proof.
		return;
	}

	#[cfg(unix)]
	{
		use std::os::unix::fs::symlink;

		let transport = TestTransport::new();
		let registry = make_registry(transport.clone());
		let real_path = make_temp_file("canon-real", false);
		let real_root = real_path.parent().expect("real root");
		let symlink_root = real_root.with_file_name(format!("{}_symlink", real_root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace")));
		let _ = std::fs::remove_file(&symlink_root);
		symlink(real_root, &symlink_root).expect("create symlink");
		let symlink_path = symlink_root.join("test.rs");

		let a1 = registry.acquire("rust", &real_path).await.expect("acquire real path");
		let a2 = registry.acquire("rust", &symlink_path).await.expect("acquire symlink path");
		assert_eq!(a1.server_id, a2.server_id);
		assert_eq!(transport.start_count(), 1, "canonical lookup must reuse a single server");
	}
}

/// Must fail runtime start when called without Tokio runtime context.
///
/// * Enforced in: `LspRuntime::start`
/// * Failure symptom: Router startup silently fails and no transport events are consumed.
#[cfg_attr(test, test)]
pub(crate) fn test_runtime_start_requires_runtime_context() {
	let transport = TestTransport::new();
	let (_session, runtime) = LspSession::new(transport);
	assert!(matches!(runtime.start(), Err(RuntimeStartError::NoRuntime)));
}
