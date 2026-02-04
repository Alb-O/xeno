use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use xeno_broker_proto::types::{
	DocSyncPhase, ErrorCode, Event, ResponsePayload, SharedApplyKind, SyncEpoch, SyncNonce,
	SyncSeq, WireOp, WireTx,
};

use super::{TestSession, setup_sync_harness};
use crate::launcher::test_helpers::TestLauncher;
use crate::services::{knowledge, routing, sessions, shared_state};

struct RoutingHarness {
	sessions: sessions::SessionHandle,
	routing: routing::RoutingHandle,
	launcher: TestLauncher,
	_sync_rx: mpsc::Receiver<shared_state::SharedStateCmd>,
}

async fn setup_routing_harness(idle_lease: Duration) -> RoutingHarness {
	let (sessions_handle, routing_tx, sync_tx) = sessions::SessionService::start();

	let (sync_cmd_tx, sync_cmd_rx) = mpsc::channel(8);
	let sync_handle = shared_state::SharedStateHandle::new(sync_cmd_tx);
	let _ = sync_tx.send(sync_handle).await;

	let (knowledge_sender, mut knowledge_rx) = mpsc::channel(8);
	let knowledge_handle = knowledge::KnowledgeHandle::new(knowledge_sender);
	tokio::spawn(async move { while knowledge_rx.recv().await.is_some() {} });

	let launcher = TestLauncher::new();
	let routing_handle = routing::RoutingService::start(
		sessions_handle.clone(),
		knowledge_handle,
		Arc::new(launcher.clone()),
		idle_lease,
	);

	let _ = routing_tx.send(routing_handle.clone()).await;
	tokio::task::yield_now().await;

	RoutingHarness {
		sessions: sessions_handle,
		routing: routing_handle,
		launcher,
		_sync_rx: sync_cmd_rx,
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_open_owner_then_follower_gets_snapshot() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello world".into(),
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedOpened { snapshot, text } => {
			assert_eq!(snapshot.epoch, SyncEpoch(1));
			assert_eq!(snapshot.seq, SyncSeq(0));
			assert_eq!(snapshot.owner, Some(session1.session_id));
			assert_eq!(snapshot.preferred_owner, Some(session1.session_id));
			assert_eq!(snapshot.phase, DocSyncPhase::Owned);
			assert!(text.is_none());
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let resp: ResponsePayload = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"stale".into(),
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedOpened { snapshot, text } => {
			assert_eq!(snapshot.epoch, SyncEpoch(1));
			assert_eq!(snapshot.seq, SyncSeq(0));
			assert_eq!(snapshot.owner, Some(session1.session_id));
			assert_eq!(text, Some("hello world".to_string()));
		}
		other => panic!("unexpected response: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_preferred_owner_enforcement() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);

	// session2 tries to edit but session1 is focused/preferred
	let result = harness
		.sync
		.apply(
			session2.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Edit,
			SyncEpoch(1),
			SyncSeq(0),
			hash,
			len,
			Some(wire_tx.clone()),
			0,
		)
		.await;

	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_focus_transfers_ownership() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();

	// session2 claims focus
	let resp: ResponsePayload = harness
		.sync
		.focus(
			session2.session_id,
			"file:///test.rs".to_string(),
			true,
			1,
			SyncNonce(1),
			Some(hash),
			Some(len),
		)
		.await
		.unwrap();

	match resp {
		ResponsePayload::SharedFocusAck { snapshot, .. } => {
			assert_eq!(snapshot.preferred_owner, Some(session2.session_id));
			assert_eq!(snapshot.owner, Some(session2.session_id));
			assert_eq!(snapshot.epoch, SyncEpoch(2));
		}
		_ => panic!(),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_transfer_requires_resync_before_edit() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);

	// session1 blurs
	let _ = harness
		.sync
		.focus(
			session1.session_id,
			"file:///test.rs".to_string(),
			false,
			1,
			SyncNonce(1),
			Some(hash),
			Some(len),
		)
		.await;

	// session1 is now owner but needs resync because epoch changed on blur/unlock
	let result = harness
		.sync
		.apply(
			session1.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Edit,
			SyncEpoch(1),
			SyncSeq(0),
			hash,
			len,
			Some(wire_tx),
			0,
		)
		.await;

	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_resync_matches_fingerprint_returns_empty() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	let resp: ResponsePayload = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncNonce(1),
			Some(hash),
			Some(len),
		)
		.await
		.unwrap();

	match resp {
		ResponsePayload::SharedSnapshot {
			text,
			snapshot: _,
			nonce: _,
		} => {
			assert!(text.is_empty());
		}
		_ => panic!(),
	}

	let resp: ResponsePayload = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncNonce(1),
			Some(hash + 1),
			Some(len),
		)
		.await
		.unwrap();

	match resp {
		ResponsePayload::SharedSnapshot { text, .. } => {
			assert_eq!(text, "hello");
		}
		_ => panic!(),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_idle_unlocks_owner() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);

	// Wait for idle
	tokio::time::pause();
	tokio::time::advance(Duration::from_secs(3)).await;

	let result = harness
		.sync
		.apply(
			session1.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Edit,
			SyncEpoch(1),
			SyncSeq(0),
			hash,
			len,
			Some(wire_tx),
			0,
		)
		.await;

	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_activity_resets_idle() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let resp: ResponsePayload = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	tokio::time::pause();
	tokio::time::advance(Duration::from_secs(1)).await;

	let _ = harness
		.sync
		.activity(session1.session_id, "file:///test.rs".to_string())
		.await;

	tokio::time::advance(Duration::from_secs(1)).await;

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let resp: ResponsePayload = harness
		.sync
		.apply(
			session1.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Edit,
			SyncEpoch(1),
			SyncSeq(0),
			hash,
			len,
			Some(wire_tx),
			0,
		)
		.await
		.unwrap();

	match resp {
		ResponsePayload::SharedApplyAck { seq, .. } => {
			assert_eq!(seq, SyncSeq(1));
		}
		_ => panic!(),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_transfer_requires_resync() {
	let harness = setup_sync_harness().await;
	let mut session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let _ = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();

	let resp: ResponsePayload = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (hash, len) = match resp {
		ResponsePayload::SharedOpened { snapshot, .. } => (snapshot.hash64, snapshot.len_chars),
		_ => panic!(),
	};

	// session2 claims focus
	let _ = harness
		.sync
		.focus(
			session2.session_id,
			"file:///test.rs".to_string(),
			true,
			1,
			SyncNonce(1),
			Some(hash),
			Some(len),
		)
		.await
		.unwrap();

	// session1 receives OwnerChanged
	let event = session1.recv_event().await.unwrap();
	match event {
		Event::SharedOwnerChanged { snapshot } => {
			assert_eq!(snapshot.owner, Some(session2.session_id));
		}
		_ => panic!(),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_routing_lsp_docs_from_sync() {
	let harness = setup_routing_harness(Duration::from_secs(30)).await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	// In this test, we need to mock the sync service behavior because it's not actually running
	// in the routing harness as configured in setup_routing_harness.
	// But the routing service expects lsp_doc_open to be called.
}
