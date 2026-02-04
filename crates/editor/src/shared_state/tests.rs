//! Tests for shared state synchronization.

use std::collections::VecDeque;

use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, SessionId, SyncEpoch, SyncNonce, SyncSeq, WireTx,
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
