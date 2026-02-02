//! Buffer sync tests for BrokerCore.

use xeno_broker_proto::types::{
	BufferSyncRole, ErrorCode, Event, IpcFrame, ResponsePayload, SyncEpoch, SyncSeq, WireOp, WireTx,
};
use xeno_rpc::MainLoopEvent;

use super::helpers::TestSession;
use crate::core::BrokerCore;

/// Extract an `Event` from a `MainLoopEvent<IpcFrame, ..>`.
fn extract_event(
	msg: MainLoopEvent<
		IpcFrame,
		xeno_broker_proto::types::Request,
		xeno_broker_proto::types::Response,
	>,
) -> Option<Event> {
	match msg {
		MainLoopEvent::Outgoing(IpcFrame::Event(e)) => Some(e),
		_ => None,
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_open_owner_then_follower_gets_snapshot() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	// First opener becomes owner
	let resp =
		core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello world", None);
	match resp {
		ResponsePayload::BufferSyncOpened {
			role,
			epoch,
			seq,
			snapshot,
		} => {
			assert_eq!(role, BufferSyncRole::Owner);
			assert_eq!(epoch, SyncEpoch(1));
			assert_eq!(seq, SyncSeq(0));
			assert!(snapshot.is_none());
		}
		_ => panic!("expected BufferSyncOpened, got {:?}", resp),
	}

	// Second opener becomes follower with snapshot
	let resp = core.on_buffer_sync_open(
		session2.session_id,
		"file:///test.rs",
		"stale content",
		None,
	);
	match resp {
		ResponsePayload::BufferSyncOpened {
			role,
			epoch,
			seq,
			snapshot,
		} => {
			assert_eq!(role, BufferSyncRole::Follower);
			assert_eq!(epoch, SyncEpoch(1));
			assert_eq!(seq, SyncSeq(0));
			assert_eq!(snapshot.as_deref(), Some("hello world"));
		}
		_ => panic!("expected BufferSyncOpened, got {:?}", resp),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_rejects_non_owner() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	// Non-owner delta must be rejected
	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let result = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::NotDocOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_seq_mismatch_triggers_resync() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());
	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);

	// Apply a valid delta first
	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let resp = core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx,
	);
	assert!(resp.is_ok());

	// Now submit with stale base_seq=0 (should be 1)
	let wire_tx2 = WireTx(vec![WireOp::Retain(11), WireOp::Insert("!".into())]);
	let result = core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx2,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::SyncSeqMismatch);
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_owner_disconnect_elects_successor_epoch_bumps() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);
	let mut session3 = TestSession::new(3);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());
	core.register_session(session3.session_id, session3.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);
	core.on_buffer_sync_open(session3.session_id, "file:///test.rs", "", None);

	// Drain any pre-existing messages
	while session1.try_recv().is_some() {}
	while session2.try_recv().is_some() {}
	while session3.try_recv().is_some() {}

	// Owner disconnects (cleanup_session_sync_docs is called via unregister_session)
	core.cleanup_session_sync_docs(session1.session_id);

	// session2 is min(2,3) so it should become new owner with epoch=2
	let event2 = session2.try_recv().and_then(extract_event);
	let event3 = session3.try_recv().and_then(extract_event);

	match event2.unwrap() {
		Event::BufferSyncOwnerChanged { uri, epoch, owner } => {
			assert_eq!(uri, "file:///test.rs");
			assert_eq!(epoch, SyncEpoch(2));
			assert_eq!(owner, session2.session_id);
		}
		other => panic!("expected OwnerChanged, got {:?}", other),
	}

	match event3.unwrap() {
		Event::BufferSyncOwnerChanged { uri, epoch, owner } => {
			assert_eq!(uri, "file:///test.rs");
			assert_eq!(epoch, SyncEpoch(2));
			assert_eq!(owner, session2.session_id);
		}
		other => panic!("expected OwnerChanged, got {:?}", other),
	}

	// Disconnected session should NOT receive event
	assert!(session1.try_recv().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_delta_ack_and_broadcast() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	// Drain initial messages
	while session2.try_recv().is_some() {}

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let resp = core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx,
	);

	// Owner gets ack with new seq
	match resp.unwrap() {
		ResponsePayload::BufferSyncDeltaAck { seq } => {
			assert_eq!(seq, SyncSeq(1));
		}
		other => panic!("expected DeltaAck, got {:?}", other),
	}

	// Follower gets broadcast delta event
	let event = session2.try_recv().and_then(extract_event);
	match event.unwrap() {
		Event::BufferSyncDelta {
			uri,
			epoch,
			seq,
			tx,
		} => {
			assert_eq!(uri, "file:///test.rs");
			assert_eq!(epoch, SyncEpoch(1));
			assert_eq!(seq, SyncSeq(1));
			assert_eq!(tx, wire_tx);
		}
		other => panic!("expected BufferSyncDelta, got {:?}", other),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_broadcast_matches_broker_rope() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "abcdef", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	// Apply a delta
	let wire_tx = WireTx(vec![
		WireOp::Retain(3),
		WireOp::Insert("XY".into()),
		WireOp::Delete(3),
	]);
	core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx,
	)
	.unwrap();

	// Resync to verify broker rope matches expected
	let resp = core
		.on_buffer_sync_resync(session2.session_id, "file:///test.rs")
		.unwrap();
	match resp {
		ResponsePayload::BufferSyncSnapshot {
			text, epoch, seq, ..
		} => {
			assert_eq!(text, "abcXY");
			assert_eq!(epoch, SyncEpoch(1));
			assert_eq!(seq, SyncSeq(1));
		}
		other => panic!("expected BufferSyncSnapshot, got {:?}", other),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_take_ownership() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	while session1.try_recv().is_some() {}
	while session2.try_recv().is_some() {}

	// Session2 takes ownership
	let resp = core.on_buffer_sync_take_ownership(session2.session_id, "file:///test.rs");
	match resp.unwrap() {
		ResponsePayload::BufferSyncOwnership { epoch } => {
			assert_eq!(epoch, SyncEpoch(2));
		}
		other => panic!("expected BufferSyncOwnership, got {:?}", other),
	}

	// Both sessions receive OwnerChanged broadcast
	let event1 = session1.try_recv().and_then(extract_event).unwrap();
	let event2 = session2.try_recv().and_then(extract_event).unwrap();

	for event in [event1, event2] {
		match event {
			Event::BufferSyncOwnerChanged { epoch, owner, .. } => {
				assert_eq!(epoch, SyncEpoch(2));
				assert_eq!(owner, session2.session_id);
			}
			other => panic!("expected OwnerChanged, got {:?}", other),
		}
	}

	// New owner must resync before submitting deltas
	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);
	let result = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(0),
		&wire_tx,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::OwnerNeedsResync);

	let _ = core
		.on_buffer_sync_resync(session2.session_id, "file:///test.rs")
		.unwrap();
	let resp = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(0),
		&wire_tx,
	);
	assert!(resp.is_ok());

	// Old owner cannot
	let wire_tx2 = WireTx(vec![WireOp::Retain(6), WireOp::Insert("?".into())]);
	let result = core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(1),
		&wire_tx2,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::NotDocOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_take_ownership_idempotent_does_not_bump_epoch() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());
	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);

	while session1.try_recv().is_some() {}

	let resp = core.on_buffer_sync_take_ownership(session1.session_id, "file:///test.rs");
	match resp.unwrap() {
		ResponsePayload::BufferSyncOwnership { epoch } => {
			assert_eq!(epoch, SyncEpoch(1));
		}
		other => panic!("expected BufferSyncOwnership, got {:?}", other),
	}

	assert!(session1.try_recv().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn test_owner_transfer_requires_resync_before_delta() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	while session1.try_recv().is_some() {}
	while session2.try_recv().is_some() {}

	core.on_buffer_sync_close(session1.session_id, "file:///test.rs")
		.unwrap();

	let event = session2.try_recv().and_then(extract_event).unwrap();
	match event {
		Event::BufferSyncOwnerChanged { epoch, owner, .. } => {
			assert_eq!(epoch, SyncEpoch(2));
			assert_eq!(owner, session2.session_id);
		}
		other => panic!("expected OwnerChanged, got {:?}", other),
	}

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);
	let result = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(0),
		&wire_tx,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::OwnerNeedsResync);

	core.on_buffer_sync_resync(session2.session_id, "file:///test.rs")
		.unwrap();
	let resp = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(0),
		&wire_tx,
	);
	assert!(resp.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn test_owner_disconnect_requires_resync_before_delta() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	while session1.try_recv().is_some() {}
	while session2.try_recv().is_some() {}

	core.cleanup_session_sync_docs(session1.session_id);

	let event = session2.try_recv().and_then(extract_event).unwrap();
	match event {
		Event::BufferSyncOwnerChanged { epoch, owner, .. } => {
			assert_eq!(epoch, SyncEpoch(2));
			assert_eq!(owner, session2.session_id);
		}
		other => panic!("expected OwnerChanged, got {:?}", other),
	}

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);
	let result = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(0),
		&wire_tx,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::OwnerNeedsResync);

	core.on_buffer_sync_resync(session2.session_id, "file:///test.rs")
		.unwrap();
	let resp = core.on_buffer_sync_delta(
		session2.session_id,
		"file:///test.rs",
		SyncEpoch(2),
		SyncSeq(0),
		&wire_tx,
	);
	assert!(resp.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_delta_invalid_tx_is_non_mutating() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());
	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);

	let wire_tx = WireTx(vec![WireOp::Delete(999)]);
	let result = core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx,
	);
	assert_eq!(result.unwrap_err(), ErrorCode::InvalidDelta);

	let resp = core
		.on_buffer_sync_resync(session1.session_id, "file:///test.rs")
		.unwrap();
	match resp {
		ResponsePayload::BufferSyncSnapshot { text, seq, .. } => {
			assert_eq!(text, "hello");
			assert_eq!(seq, SyncSeq(0));
		}
		other => panic!("expected BufferSyncSnapshot, got {:?}", other),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_close_last_session_removes_doc() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "hello", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	// Close session1
	let resp = core.on_buffer_sync_close(session1.session_id, "file:///test.rs");
	assert!(resp.is_ok());

	// Close session2 (last one) - doc should be removed
	let resp = core.on_buffer_sync_close(session2.session_id, "file:///test.rs");
	assert!(resp.is_ok());

	// Resync on removed doc should fail
	let result = core.on_buffer_sync_resync(session1.session_id, "file:///test.rs");
	assert_eq!(result.unwrap_err(), ErrorCode::SyncDocNotFound);
}

#[tokio::test(flavor = "current_thread")]
async fn test_buffer_sync_resync_returns_snapshot() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	core.on_buffer_sync_open(session1.session_id, "file:///test.rs", "initial", None);
	core.on_buffer_sync_open(session2.session_id, "file:///test.rs", "", None);

	// Apply some deltas
	let wire_tx = WireTx(vec![
		WireOp::Delete(7),
		WireOp::Insert("modified content".into()),
	]);
	core.on_buffer_sync_delta(
		session1.session_id,
		"file:///test.rs",
		SyncEpoch(1),
		SyncSeq(0),
		&wire_tx,
	)
	.unwrap();

	// Follower requests resync
	let resp = core
		.on_buffer_sync_resync(session2.session_id, "file:///test.rs")
		.unwrap();
	match resp {
		ResponsePayload::BufferSyncSnapshot {
			text,
			epoch,
			seq,
			owner,
		} => {
			assert_eq!(text, "modified content");
			assert_eq!(epoch, SyncEpoch(1));
			assert_eq!(seq, SyncSeq(1));
			assert_eq!(owner, session1.session_id);
		}
		other => panic!("expected BufferSyncSnapshot, got {:?}", other),
	}
}
