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
fn test_pending_lsp_append_respects_thresholds() {
	let mut pending = PendingLsp::new(test_config(), 0);

	for i in 0..LSP_MAX_INCREMENTAL_CHANGES + 1 {
		pending.append_changes(
			vec![LspDocumentChange {
				range: xeno_primitives::lsp::LspRange::point(
					xeno_primitives::lsp::LspPosition::new(0, 0),
				),
				new_text: "x".to_string(),
			}],
			false,
			i as u64,
		);
	}

	assert!(pending.force_full);
	assert!(pending.changes.is_empty());
}

#[test]
fn test_pending_lsp_is_due_respects_debounce() {
	let pending = PendingLsp::new(test_config(), 0);
	assert!(!pending.is_due(Instant::now(), LSP_DEBOUNCE));
}

#[test]
fn test_pending_lsp_force_full_is_due_immediately() {
	let mut pending = PendingLsp::new(test_config(), 0);
	pending.force_full = true;
	assert!(pending.is_due(Instant::now(), LSP_DEBOUNCE));
}

#[test]
fn test_pending_lsp_retry_after_delays_flush() {
	let mut pending = PendingLsp::new(test_config(), 0);
	pending.force_full = true;
	pending.retry_after = Some(Instant::now() + Duration::from_secs(1));
	assert!(!pending.is_due(Instant::now(), LSP_DEBOUNCE));
}

#[test]
fn test_pending_state_accumulate_updates_version() {
	let mut state = PendingLspState::new();

	state.accumulate(DocumentId(1), test_config(), vec![], false, 42);

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
fn test_error_forces_full_sync() {
	let mut pending = PendingLsp::new(test_config(), 0);

	assert!(!pending.force_full);
	pending.mark_error_retry();
	assert!(pending.force_full);
	assert!(pending.retry_after.is_some());
	assert!(pending.retry_after.unwrap() > Instant::now());
}

#[test]
fn test_accumulate_while_in_flight() {
	let mut state = PendingLspState::new();

	state.accumulate(
		DocumentId(1),
		test_config(),
		vec![LspDocumentChange {
			range: xeno_primitives::lsp::LspRange::point(xeno_primitives::lsp::LspPosition::new(
				0, 0,
			)),
			new_text: "a".to_string(),
		}],
		false,
		1,
	);
	state.mark_in_flight(DocumentId(1));
	state.pending.remove(&DocumentId(1));

	state.accumulate(
		DocumentId(1),
		test_config(),
		vec![LspDocumentChange {
			range: xeno_primitives::lsp::LspRange::point(xeno_primitives::lsp::LspPosition::new(
				0, 1,
			)),
			new_text: "b".to_string(),
		}],
		false,
		2,
	);

	let pending = state.pending.get(&DocumentId(1)).unwrap();
	assert_eq!(pending.changes.len(), 1);
	assert_eq!(pending.editor_version, 2);
	assert!(state.is_in_flight(&DocumentId(1)));
}

#[test]
fn test_bytes_threshold_triggers_full_sync() {
	let mut pending = PendingLsp::new(test_config(), 0);

	pending.append_changes(
		vec![LspDocumentChange {
			range: xeno_primitives::lsp::LspRange::point(xeno_primitives::lsp::LspPosition::new(
				0, 0,
			)),
			new_text: "x".repeat(LSP_MAX_INCREMENTAL_BYTES + 1),
		}],
		false,
		1,
	);

	assert!(pending.force_full);
	assert!(pending.changes.is_empty());
}

#[tokio::test]
async fn test_incremental_flush_skips_snapshot() {
	let (sync, _registry, _documents, _receiver) = DocumentSync::create();
	let metrics = Arc::new(EditorMetrics::new());
	let mut state = PendingLspState::new();

	state.accumulate(
		DocumentId(1),
		test_config(),
		vec![LspDocumentChange {
			range: xeno_primitives::lsp::LspRange::point(xeno_primitives::lsp::LspPosition::new(
				0, 0,
			)),
			new_text: "x".to_string(),
		}],
		false,
		1,
	);

	if let Some(pending) = state.pending.get_mut(&DocumentId(1)) {
		pending.last_edit_at = Instant::now() - LSP_DEBOUNCE;
	}

	let snapshot_calls = Arc::new(AtomicUsize::new(0));
	let snapshot_calls_clone = snapshot_calls.clone();

	let stats = state.flush_due(
		Instant::now(),
		LSP_DEBOUNCE,
		LSP_MAX_DOCS_PER_TICK,
		&sync,
		&metrics,
		|_| {
			snapshot_calls_clone.fetch_add(1, Ordering::Relaxed);
			Some(Rope::from_str(""))
		},
	);

	assert_eq!(stats.full_syncs, 0);
	assert_eq!(snapshot_calls.load(Ordering::Relaxed), 0);
}
