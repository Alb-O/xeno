use std::path::PathBuf;
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
