use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::{sleep, timeout};
use xeno_lsp::Error as LspError;
use xeno_worker::ActorShutdownMode;

use super::*;

fn test_config() -> LspDocumentConfig {
	LspDocumentConfig {
		path: PathBuf::from("/test/file.rs"),
		language: "rust".to_string(),
		supports_incremental: true,
	}
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

#[tokio::test]
async fn test_doc_open_close() {
	let mut mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let doc_id = DocumentId(1);

	mgr.reset_tracked(doc_id, test_config(), 1);
	wait_until("tracked doc", || mgr.is_tracked(&doc_id)).await;
	assert!(mgr.is_tracked(&doc_id));
	assert_eq!(mgr.doc_count(), 1);

	mgr.on_doc_close(doc_id);
	wait_until("doc close", || !mgr.is_tracked(&doc_id)).await;
	assert!(!mgr.is_tracked(&doc_id));
	assert_eq!(mgr.doc_count(), 0);
}

fn test_change(text: &str) -> LspDocumentChange {
	LspDocumentChange {
		range: xeno_primitives::LspRange {
			start: xeno_primitives::LspPosition { line: 0, character: 0 },
			end: xeno_primitives::LspPosition { line: 0, character: 0 },
		},
		new_text: text.to_string(),
	}
}

#[tokio::test]
async fn test_record_changes() {
	let mut mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let doc_id = DocumentId(1);
	mgr.reset_tracked(doc_id, test_config(), 1);

	mgr.on_doc_edit(doc_id, 1, 2, vec![test_change("hello")], 5);
	wait_until("pending count", || mgr.pending_count() == 1).await;

	assert_eq!(mgr.pending_count(), 1);
}

#[tokio::test]
async fn test_actor_restarts_after_failure_and_recovers() {
	let mut mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let before = mgr.restart_count();
	mgr.crash_for_test();
	wait_until("lsp.sync restart", || mgr.restart_count() > before).await;

	let doc_id = DocumentId(42);
	mgr.reset_tracked(doc_id, test_config(), 1);
	wait_until("tracked after restart", || mgr.is_tracked(&doc_id)).await;
	assert!(mgr.is_tracked(&doc_id));
}

#[tokio::test]
async fn test_shutdown_returns_completed_report() {
	let mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let report = mgr
		.shutdown(ActorShutdownMode::Graceful {
			timeout: Duration::from_millis(200),
		})
		.await;
	assert!(report.actor.completed);
	assert!(!report.actor.timed_out);
}

#[test]
fn test_threshold_escalation() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;

	for i in 0..LSP_MAX_INCREMENTAL_CHANGES + 1 {
		let prev = i as u64 + 1;
		let new = i as u64 + 2;
		state.record_changes(prev, new, vec![test_change("x")], 1);
	}

	assert!(state.needs_full);
	assert!(state.pending_changes.is_empty());
}

#[test]
fn test_flush_result_error_classification() {
	assert_eq!(FlushResult::from_error(&LspError::Backpressure), FlushResult::Retryable);
	assert_eq!(FlushResult::from_error(&LspError::NotReady), FlushResult::Retryable);
	assert_eq!(FlushResult::from_error(&LspError::Protocol("test".into())), FlushResult::Failed);
	assert_eq!(FlushResult::from_error(&LspError::ServiceStopped), FlushResult::Failed);
}

#[test]
fn test_retryable_error_does_not_escalate() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.phase = SyncPhase::InFlight;

	state.mark_complete(FlushResult::Retryable, false);

	assert!(!state.needs_full);
	assert!(state.retry_after.is_some());
	assert_eq!(state.phase, SyncPhase::Debouncing);
}

#[test]
fn test_failed_error_escalates_to_full() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.phase = SyncPhase::InFlight;

	state.mark_complete(FlushResult::Failed, false);

	assert!(state.needs_full);
	assert!(state.retry_after.is_some());
	assert_eq!(state.phase, SyncPhase::Debouncing);
}

#[test]
fn test_success_clears_retry_state() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.phase = SyncPhase::InFlight;
	state.retry_after = Some(Instant::now() + Duration::from_secs(1));

	state.mark_complete(FlushResult::Success, false);

	assert!(!state.needs_full);
	assert!(state.retry_after.is_none());
	assert_eq!(state.phase, SyncPhase::Idle);
}

#[test]
fn test_write_timeout_escalates_to_full() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.phase = SyncPhase::InFlight;
	state.inflight = Some(InFlightInfo {
		is_full: false,
		version: 5,
		started_at: Instant::now() - LSP_WRITE_TIMEOUT - Duration::from_secs(1),
	});

	let timed_out = state.check_write_timeout(Instant::now(), LSP_WRITE_TIMEOUT);

	assert!(timed_out);
	assert!(state.needs_full);
	assert!(state.inflight.is_none());
	assert!(state.retry_after.is_some());
	assert_eq!(state.phase, SyncPhase::Debouncing);
}

#[test]
fn test_no_timeout_when_recent() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.phase = SyncPhase::InFlight;
	state.inflight = Some(InFlightInfo {
		is_full: false,
		version: 5,
		started_at: Instant::now() - Duration::from_millis(100),
	});

	let timed_out = state.check_write_timeout(Instant::now(), LSP_WRITE_TIMEOUT);

	assert!(!timed_out);
	assert!(!state.needs_full);
	assert!(state.inflight.is_some());
}

#[test]
fn test_contiguity_check_success() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;

	// First commit establishes baseline
	state.record_changes(1, 2, vec![test_change("a")], 1);
	assert!(!state.needs_full);
	assert_eq!(state.expected_prev, Some(2));

	// Contiguous commits work
	state.record_changes(2, 3, vec![test_change("b")], 1);
	assert!(!state.needs_full);
	assert_eq!(state.expected_prev, Some(3));

	state.record_changes(3, 4, vec![test_change("c")], 1);
	assert!(!state.needs_full);
	assert_eq!(state.expected_prev, Some(4));
	assert_eq!(state.pending_changes.len(), 3);
}

#[test]
fn test_contiguity_check_gap_triggers_full_sync() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;

	// Establish baseline
	state.record_changes(1, 2, vec![test_change("a")], 1);
	assert!(!state.needs_full);

	// Gap detected (expected 2, got 5)
	state.record_changes(5, 6, vec![test_change("b")], 1);
	assert!(state.needs_full);
	assert!(state.pending_changes.is_empty());
	assert_eq!(state.expected_prev, Some(6));
}

#[test]
fn test_contiguity_check_reorder_triggers_full_sync() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;

	// Establish baseline with version 5
	state.record_changes(4, 5, vec![test_change("a")], 1);
	assert!(!state.needs_full);
	assert_eq!(state.expected_prev, Some(5));

	// Reorder detected (expected 5, got 3 - stale commit)
	state.record_changes(3, 4, vec![test_change("b")], 1);
	assert!(state.needs_full);
	assert!(state.pending_changes.is_empty());
}

#[test]
fn test_full_sync_resets_expected_prev() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.expected_prev = Some(5);
	state.editor_version = 10;
	state.phase = SyncPhase::InFlight;

	// Successful full sync resets expected_prev to current editor_version
	state.mark_complete(FlushResult::Success, true);

	assert_eq!(state.expected_prev, Some(10));
	assert_eq!(state.phase, SyncPhase::Idle);
}

#[test]
fn test_escalate_full_forces_next_send_to_full() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.open_sent = true;

	// Record incremental changes.
	state.record_changes(1, 2, vec![test_change("a")], 1);
	assert!(!state.needs_full);
	assert!(!state.pending_changes.is_empty());

	// Simulate didChange transport failure → escalate_full.
	state.escalate_full();
	assert!(state.needs_full);
	assert!(state.pending_changes.is_empty(), "escalate_full must clear pending incrementals");

	// is_due should return true immediately (full syncs bypass debounce).
	let far_future = Instant::now() + Duration::from_secs(0);
	assert!(state.is_due(far_future, Duration::from_millis(100)));

	// take_for_send with is_full=true clears the flag.
	let (changes, _bytes) = state.take_for_send(true);
	assert!(!state.needs_full, "needs_full must be cleared after full send");
	assert!(changes.is_empty(), "full send should have no incremental changes");
}

#[test]
fn test_after_full_recovery_incremental_resumes() {
	let mut state = DocSyncState::new(test_config(), 1);
	state.needs_full = false;
	state.open_sent = true;

	// Record change, then escalate (simulating didChange failure).
	state.record_changes(1, 2, vec![test_change("a")], 1);
	state.escalate_full();
	assert!(state.needs_full);

	// Full send.
	let _changes = state.take_for_send(true);
	assert!(!state.needs_full);

	// Complete the full send successfully.
	state.mark_complete(FlushResult::Success, true);
	assert_eq!(state.phase, SyncPhase::Idle);

	// Record new incremental changes.
	state.record_changes(2, 3, vec![test_change("b")], 1);
	assert!(!state.needs_full, "needs_full should stay false after recovery");
	assert_eq!(state.pending_changes.len(), 1, "incremental changes should accumulate normally");

	// take_for_send with is_full=false (incremental).
	let (changes, _bytes) = state.take_for_send(false);
	assert_eq!(changes.len(), 1, "should send the incremental change");
	assert!(!state.needs_full);
}

/// Recorded notification for e2e transport tests.
#[derive(Debug, Clone)]
struct RecordedNotification {
	method: String,
	/// `Some(true)` = full-text (no range), `Some(false)` = incremental, `None` = non-didChange.
	is_full_change: Option<bool>,
}

/// Transport that handles initialize + records notifications with fail injection.
struct E2eTransport {
	notifications: std::sync::Mutex<Vec<RecordedNotification>>,
	next_slot: std::sync::atomic::AtomicU32,
	fail_methods: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl E2eTransport {
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

	fn clear_recordings(&self) {
		self.notifications.lock().unwrap().clear();
	}
}

#[async_trait::async_trait]
impl xeno_lsp::client::LspTransport for E2eTransport {
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
			method: notif.method.clone(),
			is_full_change,
		});
		if self.fail_methods.lock().unwrap().contains(&notif.method) {
			return Err(xeno_lsp::Error::Protocol(format!("injected failure for {}", notif.method)));
		}
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
		_timeout: Option<Duration>,
	) -> xeno_lsp::Result<xeno_lsp::AnyResponse> {
		Ok(xeno_lsp::AnyResponse::new_ok(
			xeno_lsp::RequestId::Number(1),
			serde_json::to_value(xeno_lsp::lsp_types::InitializeResult {
				capabilities: xeno_lsp::lsp_types::ServerCapabilities::default(),
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

#[tokio::test]
async fn test_e2e_failed_incremental_triggers_full_then_incremental_resumes() {
	let worker_runtime = xeno_worker::WorkerRuntime::new();
	let transport = Arc::new(E2eTransport::new());
	let (sync, registry, _documents, _receiver) = xeno_lsp::DocumentSync::create(transport.clone(), worker_runtime.clone());
	let metrics = Arc::new(crate::metrics::EditorMetrics::new());

	// Register a language server so acquire() succeeds.
	registry.register(
		"rust",
		xeno_lsp::LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = PathBuf::from("/e2e_recovery.rs");
	let doc_id = DocumentId(1);

	// Open doc through DocumentSync to trigger server initialization.
	sync.open_document(&path, "rust", &ropey::Rope::from("fn main() {}")).await.unwrap();

	// Wait for initialization.
	let client = registry.get("rust", &path).unwrap();
	for _ in 0..200 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "server must be initialized");

	// Set up sync manager tracking.
	let mut mgr = LspSyncManager::new(worker_runtime);
	let config = LspDocumentConfig {
		path: path.clone(),
		language: "rust".to_string(),
		supports_incremental: true,
	};
	mgr.reset_tracked(doc_id, config, 1);
	wait_until("tracked", || mgr.is_tracked(&doc_id)).await;

	// DocSyncState starts with needs_full=true. Do an initial full sync to clear it.
	let initial_snapshot = ropey::Rope::from("fn main() {}");
	let initial_bytes = initial_snapshot.len_bytes() as u64;
	let done_rx = mgr
		.flush_now(Instant::now(), doc_id, &sync, &metrics, Some((initial_snapshot, initial_bytes)))
		.unwrap();
	timeout(Duration::from_secs(5), done_rx)
		.await
		.expect("initial flush timed out")
		.expect("initial flush oneshot dropped");
	wait_until("initial flush done", || mgr.in_flight_count() == 0).await;
	transport.clear_recordings();

	// Record an incremental edit.
	mgr.on_doc_edit(doc_id, 1, 2, vec![test_change("a")], 1);
	wait_until("pending", || mgr.pending_count() == 1).await;

	// Make didChange fail.
	transport.set_fail_method("textDocument/didChange");

	// Flush — incremental send will fail, SendComplete(Failed) escalates to full.
	let done_rx = mgr.flush_now(Instant::now(), doc_id, &sync, &metrics, None).unwrap();
	timeout(Duration::from_secs(5), done_rx)
		.await
		.expect("failing flush timed out")
		.expect("failing flush oneshot dropped");
	wait_until("failing flush processed", || mgr.in_flight_count() == 0).await;

	// Assert: the failed attempt was incremental.
	let recs = transport.recorded();
	let did_changes: Vec<_> = recs.iter().filter(|r| r.method == "textDocument/didChange").collect();
	assert!(!did_changes.is_empty(), "expected incremental didChange attempt");
	assert_eq!(
		did_changes[0].is_full_change,
		Some(false),
		"first attempt must be incremental before failure escalation"
	);

	// Clear fail + recordings. Prepare recovery.
	transport.clear_fail_method("textDocument/didChange");
	transport.clear_recordings();

	// Record another edit and flush with a full snapshot.
	// Use a future `now` to bypass retry_after.
	mgr.on_doc_edit(doc_id, 2, 3, vec![test_change("b")], 1);
	wait_until("pending after escalation", || mgr.pending_count() >= 1).await;

	let snapshot = ropey::Rope::from("fn main() { recovered }");
	let snapshot_bytes = snapshot.len_bytes() as u64;
	let far_future = Instant::now() + Duration::from_secs(10);
	let done_rx = mgr.flush_now(far_future, doc_id, &sync, &metrics, Some((snapshot, snapshot_bytes))).unwrap();
	timeout(Duration::from_secs(5), done_rx)
		.await
		.expect("recovery flush timed out")
		.expect("recovery flush oneshot dropped");
	wait_until("recovery flush done", || mgr.in_flight_count() == 0).await;

	// Assert: the recovery didChange was full-text.
	let recs = transport.recorded();
	let did_changes: Vec<_> = recs.iter().filter(|r| r.method == "textDocument/didChange").collect();
	assert!(!did_changes.is_empty(), "expected full-text didChange; got: {:?}", recs);
	assert_eq!(did_changes[0].is_full_change, Some(true), "recovery didChange must be full-text");

	// Clear and flush another incremental edit.
	transport.clear_recordings();
	mgr.on_doc_edit(doc_id, 3, 4, vec![test_change("c")], 1);
	wait_until("pending incremental", || mgr.pending_count() >= 1).await;

	let far_future = Instant::now() + Duration::from_secs(10);
	let done_rx = mgr.flush_now(far_future, doc_id, &sync, &metrics, None).unwrap();
	timeout(Duration::from_secs(5), done_rx)
		.await
		.expect("incremental flush timed out")
		.expect("incremental flush oneshot dropped");
	wait_until("incremental flush done", || mgr.in_flight_count() == 0).await;

	// Assert: the didChange was incremental (recovery complete, normal mode).
	let recs = transport.recorded();
	let did_changes: Vec<_> = recs.iter().filter(|r| r.method == "textDocument/didChange").collect();
	assert!(!did_changes.is_empty(), "expected incremental didChange; got: {:?}", recs);
	assert_eq!(did_changes[0].is_full_change, Some(false), "post-recovery didChange must be incremental");
}
