use std::path::PathBuf;
use std::time::Duration;

use xeno_lsp::Error as LspError;

use super::*;

fn test_config() -> LspDocumentConfig {
	LspDocumentConfig {
		path: PathBuf::from("/test/file.rs"),
		language: "rust".to_string(),
		supports_incremental: true,
	}
}

#[test]
fn test_doc_open_close() {
	let mut mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let doc_id = DocumentId(1);

	mgr.reset_tracked(doc_id, test_config(), 1);
	assert!(mgr.is_tracked(&doc_id));
	assert_eq!(mgr.doc_count(), 1);

	mgr.on_doc_close(doc_id);
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

#[test]
fn test_record_changes() {
	let mut mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let doc_id = DocumentId(1);
	mgr.reset_tracked(doc_id, test_config(), 1);

	mgr.on_doc_edit(doc_id, 1, 2, vec![test_change("hello")], 5);

	let state = mgr.docs.get(&doc_id).unwrap();
	assert_eq!(state.pending_changes.len(), 1);
	assert_eq!(state.pending_bytes, 5);
	assert_eq!(state.editor_version, 2);
	assert_eq!(state.phase, SyncPhase::Debouncing);
}

#[test]
fn test_threshold_escalation() {
	let mut mgr = LspSyncManager::new(xeno_worker::WorkerRuntime::new());
	let doc_id = DocumentId(1);
	mgr.reset_tracked(doc_id, test_config(), 1);

	for i in 0..LSP_MAX_INCREMENTAL_CHANGES + 1 {
		let prev = i as u64 + 1;
		let new = i as u64 + 2;
		mgr.on_doc_edit(doc_id, prev, new, vec![test_change("x")], 1);
	}

	let state = mgr.docs.get(&doc_id).unwrap();
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
