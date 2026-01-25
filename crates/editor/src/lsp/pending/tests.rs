use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

fn test_config() -> LspDocumentConfig {
	LspDocumentConfig {
		path: PathBuf::from("test.rs"),
		language: "rust".to_string(),
		supports_incremental: true,
		encoding: OffsetEncoding::Utf16,
	}
}

#[test]
fn test_pending_lsp_is_due_respects_debounce() {
	let pending = PendingLsp::new(test_config(), 0);
	// Not due immediately without force_full
	assert!(!pending.is_due(Instant::now(), LSP_DEBOUNCE, false));
}

#[test]
fn test_pending_lsp_force_full_is_due_immediately() {
	let pending = PendingLsp::new(test_config(), 0);
	// Due immediately when force_full is true
	assert!(pending.is_due(Instant::now(), LSP_DEBOUNCE, true));
}

#[test]
fn test_pending_lsp_retry_after_delays_flush() {
	let mut pending = PendingLsp::new(test_config(), 0);
	pending.retry_after = Some(Instant::now() + Duration::from_secs(1));
	// Not due even with force_full when retry_after is in the future
	assert!(!pending.is_due(Instant::now(), LSP_DEBOUNCE, true));
}

#[test]
fn test_pending_state_accumulate_updates_version() {
	let mut state = PendingLspState::new();

	state.accumulate(DocumentId(1), test_config(), 42);

	assert_eq!(
		state.pending.get(&DocumentId(1)).unwrap().editor_version,
		42
	);
}

#[test]
fn test_single_flight_sends_tracked() {
	let mut state = PendingLspState::new();

	assert!(!state.in_flight.contains(&DocumentId(1)));

	state.mark_in_flight(DocumentId(1));
	assert!(state.in_flight.contains(&DocumentId(1)));
	assert!(state.is_in_flight(&DocumentId(1)));

	state.clear_in_flight(&DocumentId(1));
	assert!(!state.in_flight.contains(&DocumentId(1)));
}

#[test]
fn test_error_marks_retry() {
	let mut pending = PendingLsp::new(test_config(), 0);

	assert!(pending.retry_after.is_none());
	pending.mark_error_retry();
	assert!(pending.retry_after.is_some());
	assert!(pending.retry_after.unwrap() > Instant::now());
}

#[test]
fn test_accumulate_while_in_flight() {
	let mut state = PendingLspState::new();

	state.accumulate(DocumentId(1), test_config(), 1);
	state.mark_in_flight(DocumentId(1));
	state.pending.remove(&DocumentId(1));

	// Accumulate again while in-flight
	state.accumulate(DocumentId(1), test_config(), 2);

	let pending = state.pending.get(&DocumentId(1)).unwrap();
	assert_eq!(pending.editor_version, 2);
	assert!(state.is_in_flight(&DocumentId(1)));
}

#[test]
fn test_touch_updates_timing() {
	let mut pending = PendingLsp::new(test_config(), 0);
	let initial_time = pending.last_edit_at;

	std::thread::sleep(Duration::from_millis(1));
	pending.touch(42);

	assert!(pending.last_edit_at > initial_time);
	assert_eq!(pending.editor_version, 42);
}

#[tokio::test]
async fn test_incremental_flush_uses_changes() {
	let (sync, _registry, _documents, _receiver) = DocumentSync::create();
	let metrics = Arc::new(EditorMetrics::new());
	let mut state = PendingLspState::new();

	state.accumulate(DocumentId(1), test_config(), 1);

	if let Some(pending) = state.pending.get_mut(&DocumentId(1)) {
		pending.last_edit_at = Instant::now() - LSP_DEBOUNCE;
	}

	let provider_calls = Arc::new(AtomicUsize::new(0));
	let provider_calls_clone = provider_calls.clone();

	let stats = state.flush_due(
		Instant::now(),
		LSP_DEBOUNCE,
		LSP_MAX_DOCS_PER_TICK,
		&sync,
		&metrics,
		|_| {
			provider_calls_clone.fetch_add(1, Ordering::Relaxed);
			Some(DocumentLspData {
				content: Rope::from_str("test content"),
				changes: vec![LspDocumentChange {
					range: xeno_primitives::lsp::LspRange::point(
						xeno_primitives::lsp::LspPosition::new(0, 0),
					),
					new_text: "x".to_string(),
				}],
				change_bytes: 1,
				force_full: false,
			})
		},
	);

	// Provider should be called once
	assert_eq!(provider_calls.load(Ordering::Relaxed), 1);
	// Should do incremental sync since changes provided and not force_full
	assert_eq!(stats.incremental_syncs, 1);
	assert_eq!(stats.full_syncs, 0);
}

#[tokio::test]
async fn test_full_sync_when_force_full() {
	let (sync, _registry, _documents, _receiver) = DocumentSync::create();
	let metrics = Arc::new(EditorMetrics::new());
	let mut state = PendingLspState::new();

	state.accumulate(DocumentId(1), test_config(), 1);

	if let Some(pending) = state.pending.get_mut(&DocumentId(1)) {
		pending.last_edit_at = Instant::now() - LSP_DEBOUNCE;
	}

	let stats = state.flush_due(
		Instant::now(),
		LSP_DEBOUNCE,
		LSP_MAX_DOCS_PER_TICK,
		&sync,
		&metrics,
		|_| {
			Some(DocumentLspData {
				content: Rope::from_str("test content"),
				changes: vec![],
				change_bytes: 0,
				force_full: true, // Force full sync
			})
		},
	);

	// Should do full sync since force_full is true
	assert_eq!(stats.full_syncs, 1);
	assert_eq!(stats.incremental_syncs, 0);
}
