use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use ropey::Rope;
use tokio::sync::mpsc;
use xeno_broker_proto::types::{ResponsePayload, SharedApplyKind, SyncEpoch, SyncSeq, WireOp, WireTx};

use super::TestSession;
use crate::core::db;
use crate::services::{knowledge, routing, sessions, shared_state};

struct SyncHarness {
	sessions: sessions::SessionHandle,
	sync: shared_state::SharedStateHandle,
	open_docs: Arc<Mutex<HashSet<String>>>,
	_routing_rx: mpsc::Receiver<routing::RoutingCmd>,
	_db_temp: tempfile::TempDir,
}

async fn setup_sync_harness() -> SyncHarness {
	let (sessions_handle, routing_tx, sync_tx) = sessions::SessionService::start();

	let (dummy_routing_tx, dummy_routing_rx) = mpsc::channel(8);
	let dummy_routing = routing::RoutingHandle::new(dummy_routing_tx);
	let _ = routing_tx.send(dummy_routing.clone()).await;

	let db_temp = tempfile::tempdir().expect("temp db dir");
	let db = db::BrokerDb::open(db_temp.path().join("broker")).expect("open broker db");

	let (sync, open_docs, knowledge_tx, sync_routing_tx) =
		shared_state::SharedStateService::start(sessions_handle.clone(), Some(db.storage()));

	let (knowledge_sender, mut knowledge_rx) = mpsc::channel(8);
	let knowledge = knowledge::KnowledgeHandle::new(knowledge_sender);
	let _ = knowledge_tx.send(knowledge.clone()).await;
	tokio::spawn(async move { while knowledge_rx.recv().await.is_some() {} });

	let _ = sync_tx.send(sync.clone()).await;
	let _ = sync_routing_tx.send(dummy_routing).await;
	tokio::task::yield_now().await;

	SyncHarness {
		sessions: sessions_handle,
		sync,
		open_docs,
		_routing_rx: dummy_routing_rx,
		_db_temp: db_temp,
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_undo_redo_roundtrip() {
	let harness = setup_sync_harness().await;
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let resp = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	let (mut hash, mut len) = match resp {
		ResponsePayload::SharedOpened { snapshot, text } => {
			assert_eq!(snapshot.epoch, SyncEpoch(1));
			assert_eq!(snapshot.seq, SyncSeq(0));
			assert!(text.is_none());
			(snapshot.hash64, snapshot.len_chars)
		}
		other => panic!("unexpected response: {other:?}"),
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

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let resp = harness
		.sync
		.apply(
			session1.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Edit,
			SyncEpoch(1),
			SyncSeq(0),
			hash,
			len,
			Some(wire_tx.clone()),
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedApplyAck {
			seq,
			hash64,
			len_chars,
			..
		} => {
			assert_eq!(seq, SyncSeq(1));
			hash = hash64;
			len = len_chars;
		}
		other => panic!("unexpected response: {other:?}"),
	}

	// session2 receives Edit delta
	let _ = session2.recv_event().await.expect("edit delta");

	let resp = harness
		.sync
		.apply(
			session1.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Undo,
			SyncEpoch(1),
			SyncSeq(1),
			hash,
			len,
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedApplyAck {
			seq,
			hash64,
			len_chars,
			applied_tx,
			..
		} => {
			assert_eq!(seq, SyncSeq(2));
			hash = hash64;
			len = len_chars;

			// Origin receives applied_tx in ack
			let tx = applied_tx.expect("undo tx in ack");
			let mut content = Rope::from("hello world");
			let tx = crate::wire_convert::wire_to_tx(&tx, content.slice(..)).unwrap();
			tx.apply(&mut content);
			assert_eq!(content.to_string(), "hello");
		}
		other => panic!("unexpected response: {other:?}"),
	}

	// Origin (session1) does NOT receive broadcast
	assert!(session1.recv_event().await.is_none());

	// Participant (session2) SHOULD receive broadcast
	let event = session2.recv_event().await.expect("undo delta broadcast");
	match event {
		Event::SharedDelta { seq, tx, .. } => {
			assert_eq!(seq, SyncSeq(2));
			let mut content = Rope::from("hello world");
			let tx = crate::wire_convert::wire_to_tx(&tx, content.slice(..)).unwrap();
			tx.apply(&mut content);
			assert_eq!(content.to_string(), "hello");
		}
		other => panic!("unexpected event: {other:?}"),
	}

	let resp = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncNonce(1),
			None,
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedSnapshot { text, .. } => {
			assert_eq!(text, "hello");
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let resp = harness
		.sync
		.apply(
			session1.session_id,
			"file:///test.rs".to_string(),
			SharedApplyKind::Redo,
			SyncEpoch(1),
			SyncSeq(2),
			hash,
			len,
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedApplyAck {
			seq,
			hash64,
			len_chars,
			applied_tx,
			..
		} => {
			assert_eq!(seq, SyncSeq(3));
			hash = hash64;
			len = len_chars;

			// Origin receives applied_tx in ack
			let tx = applied_tx.expect("redo tx in ack");
			let mut content = Rope::from("hello");
			let tx = crate::wire_convert::wire_to_tx(&tx, content.slice(..)).unwrap();
			tx.apply(&mut content);
			assert_eq!(content.to_string(), "hello world");
		}
		other => panic!("unexpected response: {other:?}"),
	}

	// session2 receives Redo delta
	let _ = session2.recv_event().await.expect("redo delta broadcast");

	let resp = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncNonce(2),
			None,
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedSnapshot { text, .. } => {
			assert_eq!(text, "hello world");
		}
		other => panic!("unexpected response: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_open_uses_db_history() {
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

	let resp = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedOpened { snapshot, text } => {
			assert_eq!(snapshot.epoch, SyncEpoch(1));
			assert_eq!(snapshot.seq, SyncSeq(0));
			assert!(text.is_none());
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let resp = harness
		.sync
		.close(session1.session_id, "file:///test.rs".to_string())
		.await
		.unwrap();
	assert!(matches!(resp, ResponsePayload::SharedClosed));

	let resp = harness
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
			assert_eq!(text.as_deref(), Some("hello"));
		}
		other => panic!("unexpected response: {other:?}"),
	}
}
