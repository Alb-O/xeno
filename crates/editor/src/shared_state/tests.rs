//! Tests for shared state synchronization.

use std::collections::VecDeque;

use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, SessionId, SharedApplyKind, SyncEpoch, SyncNonce, SyncSeq,
	WireTx,
};

use super::manager::SharedStateManager;
use super::types::{InFlightEdit, SharedDocEntry, SharedStateRole, SharedViewHistory};
use crate::buffer::DocumentId;

#[test]
fn test_snapshot_apply_when_text_present() {
	let snapshot = DocStateSnapshot {
		uri: "file:///test.rs".to_string(),
		epoch: SyncEpoch(1),
		seq: SyncSeq(2),
		owner: None,
		preferred_owner: None,
		phase: DocSyncPhase::Unlocked,
		hash64: 42,
		len_chars: 5,
		history_head_id: None,
		history_root_id: None,
		history_head_group: None,
	};

	let entry = &mut SharedDocEntry {
		doc_id: DocumentId(1),
		epoch: SyncEpoch(0),
		seq: SyncSeq(0),
		role: SharedStateRole::Follower,
		owner: None,
		preferred_owner: None,
		phase: DocSyncPhase::Unlocked,
		needs_resync: false,
		resync_requested: false,
		open_refcount: 1,
		pending_deltas: VecDeque::new(),
		pending_history: VecDeque::new(),
		in_flight: None,
		last_activity_sent: None,
		focus_seq: 0,
		next_nonce: 1,
		pending_align: None,
		auth_hash64: 0,
		auth_len_chars: 0,
		current_undo_group: 1,
		view_history: SharedViewHistory::default(),
	};

	SharedStateManager::apply_snapshot_state(entry, &snapshot, SessionId(1), true);
	assert!(!entry.needs_resync);
}

#[test]
fn test_empty_snapshot_ignored_when_doc_not_empty() {
	let mut manager = SharedStateManager::new();
	let uri = "file:///test.rs";
	manager.prepare_open(uri, "hello", DocumentId(1));
	let entry = manager.docs.get_mut(uri).unwrap();
	entry.pending_align = Some(SyncNonce(1));

	let snapshot = DocStateSnapshot {
		uri: uri.to_string(),
		epoch: SyncEpoch(1),
		seq: SyncSeq(0),
		owner: None,
		preferred_owner: None,
		phase: DocSyncPhase::Unlocked,
		hash64: 999,
		len_chars: 5,
		history_head_id: None,
		history_root_id: None,
		history_head_group: None,
	};

	let text =
		manager.handle_snapshot_response(uri, snapshot, SyncNonce(1), "".to_string(), SessionId(1));
	assert!(text.is_none());
}

#[test]
fn test_empty_snapshot_applied_when_doc_empty() {
	let mut manager = SharedStateManager::new();
	let uri = "file:///test.rs";
	manager.prepare_open(uri, "hello", DocumentId(1));
	let entry = manager.docs.get_mut(uri).unwrap();
	entry.pending_align = Some(SyncNonce(1));

	let snapshot = DocStateSnapshot {
		uri: uri.to_string(),
		epoch: SyncEpoch(1),
		seq: SyncSeq(0),
		owner: None,
		preferred_owner: None,
		phase: DocSyncPhase::Unlocked,
		hash64: 0,
		len_chars: 0,
		history_head_id: None,
		history_root_id: None,
		history_head_group: None,
	};

	let text =
		manager.handle_snapshot_response(uri, snapshot, SyncNonce(1), "".to_string(), SessionId(1));
	assert_eq!(text, Some("".to_string()));
}

#[test]
fn test_repair_text_clears_owner_pipeline() {
	let mut manager = SharedStateManager::new();
	let uri = "file:///test.rs";
	let sid = SessionId(1);
	manager.prepare_open(uri, "hello", DocumentId(1));

	let entry = manager.docs.get_mut(uri).unwrap();
	entry.role = SharedStateRole::Owner;
	entry.owner = Some(sid);
	entry.in_flight = Some(InFlightEdit {
		epoch: SyncEpoch(1),
		base_seq: SyncSeq(0),
	});
	entry.pending_deltas.push_back((WireTx(Vec::new()), 1));
	entry.pending_align = Some(SyncNonce(1));

	let snapshot = DocStateSnapshot {
		uri: uri.to_string(),
		epoch: SyncEpoch(1),
		seq: SyncSeq(0),
		owner: Some(sid),
		preferred_owner: Some(sid),
		phase: DocSyncPhase::Owned,
		hash64: 123,
		len_chars: 10,
		history_head_id: None,
		history_root_id: None,
		history_head_group: None,
	};

	let text = manager.handle_focus_ack(snapshot, SyncNonce(1), Some("repaired".to_string()), sid);

	assert_eq!(text, Some("repaired".to_string()));

	let entry = manager.docs.get(uri).unwrap();
	assert!(entry.in_flight.is_none());
	assert!(entry.pending_deltas.is_empty());
	assert!(!entry.needs_resync);
}

#[test]
fn test_snapshot_history_group_seeds_current_undo_group() {
	let mut manager = SharedStateManager::new();
	let uri = "file:///test.rs";
	manager.prepare_open(uri, "hello", DocumentId(1));

	let snapshot = DocStateSnapshot {
		uri: uri.to_string(),
		epoch: SyncEpoch(1),
		seq: SyncSeq(0),
		owner: None,
		preferred_owner: None,
		phase: DocSyncPhase::Unlocked,
		hash64: 0,
		len_chars: 5,
		history_head_id: Some(10),
		history_root_id: Some(1),
		history_head_group: Some(42),
	};

	let _ = manager.handle_opened(snapshot, None, SessionId(1));
	let entry = manager.docs.get(uri).expect("entry exists");

	assert_eq!(
		entry.current_undo_group, 42,
		"current_undo_group should seed from broker history head group"
	);
}

#[test]
fn test_apply_ack_mismatch_clears_in_flight_and_marks_resync() {
	let mut manager = SharedStateManager::new();
	let uri = "file:///test.rs";
	manager.prepare_open(uri, "hello", DocumentId(1));

	let entry = manager.docs.get_mut(uri).unwrap();
	entry.role = SharedStateRole::Owner;
	entry.epoch = SyncEpoch(1);
	entry.seq = SyncSeq(0);
	entry.in_flight = Some(InFlightEdit {
		epoch: SyncEpoch(1),
		base_seq: SyncSeq(0),
	});
	entry.needs_resync = false;

	let _ = manager.handle_apply_ack(
		uri,
		SharedApplyKind::Edit,
		SyncEpoch(1),
		SyncSeq(5),
		None,
		0,
		0,
		None,
		None,
		None,
	);

	let entry = manager.docs.get(uri).unwrap();
	assert!(
		entry.in_flight.is_none(),
		"mismatched ack should clear in_flight"
	);
	assert!(
		entry.needs_resync,
		"mismatched ack should mark needs_resync"
	);
}

#[test]
fn test_prepare_undo_queued_while_in_flight() {
	let mut manager = SharedStateManager::new();
	let uri = "file:///test.rs";
	manager.prepare_open(uri, "hello", DocumentId(1));

	let entry = manager.docs.get_mut(uri).unwrap();
	entry.role = SharedStateRole::Owner;
	entry.in_flight = Some(InFlightEdit {
		epoch: SyncEpoch(1),
		base_seq: SyncSeq(0),
	});
	entry.needs_resync = false;

	let payload = manager.prepare_undo(uri);
	assert!(
		payload.is_none(),
		"undo should be queued, not sent, while in flight"
	);
	let entry = manager.docs.get(uri).unwrap();
	assert_eq!(
		entry.pending_history.len(),
		1,
		"queued undo should be stored for later drain"
	);
}
