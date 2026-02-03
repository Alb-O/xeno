//! Service-level tests for broker actor subsystems.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ropey::Rope;
use tokio::sync::{mpsc, oneshot};
use xeno_broker_proto::types::{
	DocSyncPhase, ErrorCode, Event, IpcFrame, LspServerConfig, Request, Response, ResponsePayload,
	SessionId, SyncEpoch, SyncSeq, WireOp, WireTx,
};
use xeno_rpc::MainLoopEvent;

use super::{knowledge, routing, sessions, shared_state};
use crate::core::{SessionSink, db, normalize_uri};
use crate::launcher::test_helpers::TestLauncher;

struct TestSession {
	session_id: SessionId,
	sink: SessionSink,
	events_rx: mpsc::UnboundedReceiver<MainLoopEvent<IpcFrame, Request, Response>>,
}

impl TestSession {
	fn new(id: u64) -> Self {
		let (tx, rx) = mpsc::unbounded_channel();
		Self {
			session_id: SessionId(id),
			sink: SessionSink::from_sender(tx),
			events_rx: rx,
		}
	}

	fn try_event(&mut self) -> Option<Event> {
		self.events_rx.try_recv().ok().and_then(extract_event)
	}

	async fn recv_event(&mut self) -> Option<Event> {
		let timeout = tokio::time::timeout(Duration::from_millis(200), self.events_rx.recv());
		timeout.await.ok().flatten().and_then(extract_event)
	}
}

fn extract_event(msg: MainLoopEvent<IpcFrame, Request, Response>) -> Option<Event> {
	match msg {
		MainLoopEvent::Outgoing(IpcFrame::Event(event)) => Some(event),
		_ => None,
	}
}

fn test_config(cmd: &str, cwd: &str) -> LspServerConfig {
	LspServerConfig {
		command: cmd.to_string(),
		args: vec!["--test".to_string()],
		env: vec![],
		cwd: Some(cwd.to_string()),
	}
}

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

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
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

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let resp = harness
		.sync
		.edit(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx.clone(),
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedEditAck { seq, .. } => {
			assert_eq!(seq, SyncSeq(1));
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let resp = harness
		.sync
		.undo(session1.session_id, "file:///test.rs".to_string())
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedUndoAck { seq, .. } => {
			assert_eq!(seq, SyncSeq(2));
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let event = session1.recv_event().await.expect("undo delta");
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
		.redo(session1.session_id, "file:///test.rs".to_string())
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedRedoAck { seq, .. } => {
			assert_eq!(seq, SyncSeq(3));
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let resp = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
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

	let resp = harness
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
			assert_eq!(
				snapshot.phase,
				xeno_broker_proto::types::DocSyncPhase::Owned
			);
			assert!(text.is_none());
		}
		other => panic!("unexpected response: {other:?}"),
	}

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
			assert_eq!(snapshot.owner, Some(session1.session_id));
			assert_eq!(snapshot.preferred_owner, Some(session1.session_id));
			assert_eq!(
				snapshot.phase,
				xeno_broker_proto::types::DocSyncPhase::Owned
			);
			assert_eq!(text.as_deref(), Some("hello world"));
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

	let _ = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let result = harness
		.sync
		.edit(
			session2.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx,
		)
		.await;
	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_focus_transfers_ownership() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

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
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	while session2.try_event().is_some() {}

	let resp = harness
		.sync
		.focus(session2.session_id, "file:///test.rs".to_string(), true, 1)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedFocusAck { snapshot } => {
			assert_eq!(snapshot.epoch, SyncEpoch(2));
			assert_eq!(snapshot.owner, Some(session2.session_id));
			assert_eq!(snapshot.preferred_owner, Some(session2.session_id));
			assert_eq!(snapshot.phase, DocSyncPhase::Diverged);
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let mut saw_preferred = false;
	let mut saw_owner = false;
	for _ in 0..2 {
		let event = session2.recv_event().await.expect("focus events");
		match event {
			Event::SharedPreferredOwnerChanged { snapshot } => {
				assert_eq!(snapshot.preferred_owner, Some(session2.session_id));
				saw_preferred = true;
			}
			Event::SharedOwnerChanged { snapshot } => {
				assert_eq!(snapshot.owner, Some(session2.session_id));
				saw_owner = true;
			}
			other => panic!("unexpected event: {other:?}"),
		}
	}
	assert!(saw_preferred && saw_owner);

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);
	let result = harness
		.sync
		.edit(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx,
		)
		.await;
	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_focus_blur_unlocks() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

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
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	while session2.try_event().is_some() {}

	let resp = harness
		.sync
		.focus(session1.session_id, "file:///test.rs".to_string(), false, 1)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedFocusAck { snapshot } => {
			assert_eq!(snapshot.epoch, SyncEpoch(2));
			assert_eq!(snapshot.owner, None);
			assert_eq!(snapshot.preferred_owner, None);
			assert_eq!(snapshot.phase, DocSyncPhase::Unlocked);
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let mut saw_preferred = false;
	let mut saw_unlocked = false;
	for _ in 0..2 {
		let event = session2.recv_event().await.expect("unlock events");
		match event {
			Event::SharedPreferredOwnerChanged { snapshot } => {
				assert!(snapshot.preferred_owner.is_none());
				saw_preferred = true;
			}
			Event::SharedUnlocked { snapshot } => {
				assert_eq!(snapshot.phase, DocSyncPhase::Unlocked);
				saw_unlocked = true;
			}
			other => panic!("unexpected event: {other:?}"),
		}
	}
	assert!(saw_preferred && saw_unlocked);

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);
	let result = harness
		.sync
		.edit(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx,
		)
		.await;
	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_edit_ack_and_broadcast() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

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
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	while session2.try_event().is_some() {}

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let resp = harness
		.sync
		.edit(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx.clone(),
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedEditAck { seq, .. } => {
			assert_eq!(seq, SyncSeq(1));
		}
		other => panic!("unexpected response: {other:?}"),
	}

	let event = session2.recv_event().await.expect("delta event");
	match event {
		Event::SharedDelta {
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
		other => panic!("unexpected event: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_transfer_requires_resync_before_edit() {
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

	let _ = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	while session1.try_event().is_some() {}
	while session2.try_event().is_some() {}

	let resp = harness
		.sync
		.focus(session2.session_id, "file:///test.rs".to_string(), true, 1)
		.await
		.unwrap();
	let epoch = match resp {
		ResponsePayload::SharedFocusAck { snapshot } => {
			assert_eq!(snapshot.owner, Some(session2.session_id));
			snapshot.epoch
		}
		other => panic!("unexpected response: {other:?}"),
	};

	let mut saw_owner = false;
	while let Some(event) = session2.recv_event().await {
		match event {
			Event::SharedOwnerChanged { snapshot } => {
				assert_eq!(snapshot.owner, Some(session2.session_id));
				saw_owner = true;
				break;
			}
			Event::SharedPreferredOwnerChanged { .. } => {}
			other => panic!("unexpected event: {other:?}"),
		}
	}
	assert!(saw_owner);

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert("!".into())]);
	let result = harness
		.sync
		.edit(
			session2.session_id,
			"file:///test.rs".to_string(),
			epoch,
			SyncSeq(0),
			wire_tx.clone(),
		)
		.await;
	assert_eq!(result.unwrap_err(), ErrorCode::OwnerNeedsResync);

	harness
		.sync
		.resync(
			session2.session_id,
			"file:///test.rs".to_string(),
			None,
			None,
		)
		.await
		.unwrap();
	let resp = harness
		.sync
		.edit(
			session2.session_id,
			"file:///test.rs".to_string(),
			epoch,
			SyncSeq(0),
			wire_tx,
		)
		.await;
	assert!(resp.is_ok());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_shared_state_idle_unlocks_owner() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

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
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	while session2.try_event().is_some() {}

	tokio::time::advance(shared_state::OWNER_IDLE_TIMEOUT + Duration::from_millis(10)).await;
	tokio::task::yield_now().await;

	let event = session2.recv_event().await.expect("unlock");
	match event {
		Event::SharedUnlocked { snapshot } => {
			assert_eq!(snapshot.epoch, SyncEpoch(2));
		}
		other => panic!("unexpected event: {other:?}"),
	}

	let wire_tx = WireTx(vec![WireOp::Retain(5), WireOp::Insert(" world".into())]);
	let result = harness
		.sync
		.edit(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx,
		)
		.await;
	assert_eq!(result.unwrap_err(), ErrorCode::NotPreferredOwner);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_shared_state_activity_resets_idle_timer() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

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
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	while session2.try_event().is_some() {}

	tokio::time::advance(shared_state::OWNER_IDLE_TIMEOUT - Duration::from_secs(1)).await;
	tokio::task::yield_now().await;

	let _ = harness
		.sync
		.activity(session1.session_id, "file:///test.rs".to_string())
		.await
		.unwrap();

	let guard = shared_state::OWNER_IDLE_TIMEOUT
		.checked_sub(Duration::from_millis(100))
		.unwrap_or(Duration::from_millis(0));
	tokio::time::advance(guard).await;
	tokio::task::yield_now().await;

	assert!(session2.try_event().is_none());

	tokio::time::advance(Duration::from_millis(150)).await;
	tokio::task::yield_now().await;

	let event = session2.recv_event().await.expect("unlock after activity");
	assert!(matches!(event, Event::SharedUnlocked { .. }));
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_invalid_edit_is_non_mutating() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let _ = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await;

	let wire_tx = WireTx(vec![WireOp::Delete(999)]);
	let result = harness
		.sync
		.edit(
			session1.session_id,
			"file:///test.rs".to_string(),
			SyncEpoch(1),
			SyncSeq(0),
			wire_tx,
		)
		.await;
	assert_eq!(result.unwrap_err(), ErrorCode::InvalidDelta);

	let resp = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			None,
			None,
		)
		.await
		.unwrap();
	match resp {
		ResponsePayload::SharedSnapshot { text, snapshot } => {
			assert_eq!(text, "hello");
			assert_eq!(snapshot.seq, SyncSeq(0));
		}
		other => panic!("unexpected response: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_resync_matches_fingerprint_returns_empty() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let _ = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await;

	let (len, hash) = xeno_broker_proto::fingerprint_rope(&Rope::from_str("hello"));
	let resp = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			Some(hash),
			Some(len),
		)
		.await
		.unwrap();

	match resp {
		ResponsePayload::SharedSnapshot { text, snapshot } => {
			assert!(text.is_empty());
			assert_eq!(snapshot.hash64, hash);
			assert_eq!(snapshot.len_chars, len);
		}
		other => panic!("unexpected response: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_close_last_session_removes_doc() {
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

	let _ = harness
		.sync
		.open(
			session1.session_id,
			"file:///test.rs".to_string(),
			"hello".into(),
			None,
		)
		.await;
	let _ = harness
		.sync
		.open(
			session2.session_id,
			"file:///test.rs".to_string(),
			"".into(),
			None,
		)
		.await;

	harness
		.sync
		.close(session1.session_id, "file:///test.rs".to_string())
		.await
		.unwrap();
	harness
		.sync
		.close(session2.session_id, "file:///test.rs".to_string())
		.await
		.unwrap();

	let res = harness
		.sync
		.resync(
			session1.session_id,
			"file:///test.rs".to_string(),
			None,
			None,
		)
		.await;
	assert_eq!(res.unwrap_err(), ErrorCode::SyncDocNotFound);
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_uri_normalization_dedups() {
	let harness = setup_sync_harness().await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let uri1 = "file:///path/to/file.rs";
	let uri2 = "file://localhost/path/to/file.rs";

	let _ = harness
		.sync
		.open(
			session1.session_id,
			uri1.to_string(),
			"initial".into(),
			None,
		)
		.await;
	let _ = harness
		.sync
		.open(
			session1.session_id,
			uri2.to_string(),
			"initial".into(),
			None,
		)
		.await;

	let normalized = normalize_uri(uri1).unwrap();
	let open_docs = harness.open_docs.lock().unwrap();
	assert_eq!(open_docs.len(), 1);
	assert!(open_docs.contains(&normalized));
}

#[tokio::test(flavor = "current_thread")]
async fn test_shared_state_refcounts_keep_doc_open() {
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

	let uri = "file:///refcounts.rs";
	let _ = harness
		.sync
		.open(session1.session_id, uri.to_string(), "hello".into(), None)
		.await;
	let _ = harness
		.sync
		.open(session1.session_id, uri.to_string(), "hello".into(), None)
		.await;
	let _ = harness
		.sync
		.open(session2.session_id, uri.to_string(), "hello".into(), None)
		.await;

	harness
		.sync
		.close(session1.session_id, uri.to_string())
		.await
		.unwrap();

	let resp = harness
		.sync
		.resync(session2.session_id, uri.to_string(), None, None)
		.await;
	assert!(resp.is_ok());

	harness
		.sync
		.close(session1.session_id, uri.to_string())
		.await
		.unwrap();
	let resp = harness
		.sync
		.resync(session2.session_id, uri.to_string(), None, None)
		.await;
	assert!(resp.is_ok());

	harness
		.sync
		.close(session2.session_id, uri.to_string())
		.await
		.unwrap();
	let resp = harness
		.sync
		.resync(session2.session_id, uri.to_string(), None, None)
		.await;
	assert_eq!(resp.unwrap_err(), ErrorCode::SyncDocNotFound);
}

#[tokio::test(flavor = "current_thread")]
async fn test_s2c_registration_order() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let mut session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config)
		.await
		.unwrap();

	let (reply_tx, reply_rx) = oneshot::channel();
	let request_id = xeno_lsp::RequestId::Number(1);
	harness
		.routing
		.begin_s2c(
			server_id,
			request_id.clone(),
			"{\"method\":\"x\"}".into(),
			reply_tx,
		)
		.await
		.unwrap();

	let event = session1.recv_event().await.expect("s2c event");
	match event {
		Event::LspRequest { server_id: sid, .. } => assert_eq!(sid, server_id),
		other => panic!("unexpected event: {other:?}"),
	}

	let reply = Ok(serde_json::json!({"applied": true}));
	let completed = harness
		.routing
		.complete_s2c(session1.session_id, server_id, request_id, reply)
		.await;
	assert!(completed);

	let result = reply_rx.await.expect("reply delivered");
	assert_eq!(result.unwrap(), serde_json::json!({"applied": true}));
}

#[tokio::test(flavor = "current_thread")]
async fn test_routing_leader_selection_and_delivery() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let mut session1 = TestSession::new(10);
	let mut session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();
	let server_id2 = harness
		.routing
		.lsp_start(session2.session_id, config)
		.await
		.unwrap();
	assert_eq!(server_id2, server_id);

	let (reply_tx, _reply_rx) = oneshot::channel();
	let request_id = xeno_lsp::RequestId::Number(1);
	harness
		.routing
		.begin_s2c(server_id, request_id, "{\"method\":\"x\"}".into(), reply_tx)
		.await
		.unwrap();

	assert!(session2.recv_event().await.is_some());
	assert!(session1.try_event().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn test_routing_diagnostics_replay_on_attach() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();

	let did_open = serde_json::json!({
		"method": "textDocument/didOpen",
		"params": {
			"textDocument": {
				"uri": "file:///test.rs",
				"languageId": "rust",
				"version": 1,
				"text": "test content"
			}
		}
	})
	.to_string();

	harness
		.routing
		.lsp_send_notif(session1.session_id, server_id, did_open)
		.await
		.unwrap();

	let diag = xeno_lsp::Message::Notification(xeno_lsp::AnyNotification::new(
		"textDocument/publishDiagnostics",
		serde_json::json!({
			"uri": "file:///test.rs",
			"diagnostics": []
		}),
	));
	let diag_message = serde_json::to_string(&diag).unwrap();
	harness.routing.server_notif(server_id, diag_message).await;

	let event = session1.recv_event().await.expect("diag event");
	match event {
		Event::LspDiagnostics { uri, .. } => assert_eq!(uri, "file:///test.rs"),
		other => panic!("unexpected event: {other:?}"),
	}

	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;
	let _server_id2 = harness
		.routing
		.lsp_start(session2.session_id, config)
		.await
		.unwrap();

	let event = session2.recv_event().await.expect("replayed diag");
	match event {
		Event::LspDiagnostics { uri, .. } => assert_eq!(uri, "file:///test.rs"),
		other => panic!("unexpected event: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_routing_diagnostics_replay_after_session_lost() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();

	let did_open = serde_json::json!({
		"method": "textDocument/didOpen",
		"params": {
			"textDocument": {
				"uri": "file:///test.rs",
				"languageId": "rust",
				"version": 1,
				"text": "test content"
			}
		}
	})
	.to_string();

	harness
		.routing
		.lsp_send_notif(session1.session_id, server_id, did_open)
		.await
		.unwrap();

	let diag = xeno_lsp::Message::Notification(xeno_lsp::AnyNotification::new(
		"textDocument/publishDiagnostics",
		serde_json::json!({
			"uri": "file:///test.rs",
			"diagnostics": []
		}),
	));
	let diag_message = serde_json::to_string(&diag).unwrap();
	harness.routing.server_notif(server_id, diag_message).await;

	let event = session1.recv_event().await.expect("diag event");
	match event {
		Event::LspDiagnostics { uri, .. } => assert_eq!(uri, "file:///test.rs"),
		other => panic!("unexpected event: {other:?}"),
	}

	harness.routing.session_lost(session1.session_id).await;
	tokio::task::yield_now().await;

	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;
	let _server_id2 = harness
		.routing
		.lsp_start(session2.session_id, config)
		.await
		.unwrap();

	let event = session2.recv_event().await.expect("replayed diag");
	match event {
		Event::LspDiagnostics { uri, .. } => assert_eq!(uri, "file:///test.rs"),
		other => panic!("unexpected event: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread")]
async fn test_routing_lsp_docs_from_sync() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config)
		.await
		.unwrap();

	let did_open = serde_json::json!({
		"method": "textDocument/didOpen",
		"params": {
			"textDocument": {
				"uri": "file:///test.rs",
				"languageId": "rust",
				"version": 1,
				"text": "let a = 1;"
			}
		}
	})
	.to_string();
	harness
		.routing
		.lsp_send_notif(session1.session_id, server_id, did_open)
		.await
		.unwrap();

	let handle = harness
		.launcher
		.get_server(server_id)
		.expect("server handle");

	let wait_for = |method: &'static str| {
		let handle = handle.clone();
		async move {
			tokio::time::timeout(Duration::from_secs(1), async {
				loop {
					let found = {
						let received = handle.received.lock().unwrap();
						received.iter().any(|msg| match msg {
							xeno_lsp::Message::Notification(n) => n.method == method,
							_ => false,
						})
					};
					if found {
						break;
					}
					tokio::time::sleep(Duration::from_millis(10)).await;
				}
			})
			.await
			.expect("timeout waiting for LSP notification");
		}
	};

	wait_for("textDocument/didOpen").await;

	harness
		.routing
		.lsp_doc_update("file:///test.rs".to_string(), "let a = 2;".to_string())
		.await;
	wait_for("textDocument/didChange").await;

	harness.routing.session_lost(session1.session_id).await;
	tokio::task::yield_now().await;

	harness
		.routing
		.lsp_doc_close("file:///test.rs".to_string())
		.await;
	wait_for("textDocument/didClose").await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_routing_session_lost_closes_lsp_docs() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config)
		.await
		.unwrap();

	let did_open = serde_json::json!({
		"method": "textDocument/didOpen",
		"params": {
			"textDocument": {
				"uri": "file:///test.rs",
				"languageId": "rust",
				"version": 1,
				"text": "let a = 1;"
			}
		}
	})
	.to_string();
	harness
		.routing
		.lsp_send_notif(session1.session_id, server_id, did_open)
		.await
		.unwrap();

	let handle = harness
		.launcher
		.get_server(server_id)
		.expect("server handle");

	let wait_for = |method: &'static str| {
		let handle = handle.clone();
		async move {
			tokio::time::timeout(Duration::from_secs(1), async {
				loop {
					let found = {
						let received = handle.received.lock().unwrap();
						received.iter().any(|msg| match msg {
							xeno_lsp::Message::Notification(n) => n.method == method,
							_ => false,
						})
					};
					if found {
						break;
					}
					tokio::time::sleep(Duration::from_millis(10)).await;
				}
			})
			.await
			.expect("timeout waiting for LSP notification");
		}
	};

	wait_for("textDocument/didOpen").await;

	harness.routing.session_lost(session1.session_id).await;
	tokio::task::yield_now().await;

	wait_for("textDocument/didClose").await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_session_lost_cancels_pending_and_reselects_leader() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
	let session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;
	harness
		.sessions
		.register(session2.session_id, session2.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();
	let _ = harness
		.routing
		.lsp_start(session2.session_id, config)
		.await
		.unwrap();

	let (reply_tx, reply_rx) = oneshot::channel();
	let request_id = xeno_lsp::RequestId::Number(1);
	harness
		.routing
		.begin_s2c(
			server_id,
			request_id.clone(),
			"{\"method\":\"x\"}".into(),
			reply_tx,
		)
		.await
		.unwrap();

	harness.routing.session_lost(session1.session_id).await;
	tokio::task::yield_now().await;
	let result = reply_rx.await.expect("pending cancelled");
	assert!(
		matches!(result, Err(ref e) if e.code == xeno_lsp::ErrorCode::REQUEST_CANCELLED),
		"unexpected result: {result:?}"
	);

	let (reply_tx2, _reply_rx2) = oneshot::channel();
	let request_id2 = xeno_lsp::RequestId::Number(2);
	harness
		.routing
		.begin_s2c(
			server_id,
			request_id2,
			"{\"method\":\"x\"}".into(),
			reply_tx2,
		)
		.await
		.unwrap();

	assert!(session2.recv_event().await.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn test_server_exit_cancellation() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
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

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();
	let _ = harness
		.routing
		.lsp_start(session2.session_id, config)
		.await
		.unwrap();

	let (reply_tx, reply_rx) = oneshot::channel();
	let request_id = xeno_lsp::RequestId::Number(1);
	harness
		.routing
		.begin_s2c(server_id, request_id, "{\"method\":\"x\"}".into(), reply_tx)
		.await
		.unwrap();
	let event = session1.recv_event().await.expect("s2c event");
	match event {
		Event::LspRequest { server_id: sid, .. } => assert_eq!(sid, server_id),
		other => panic!("unexpected event: {other:?}"),
	}

	harness.routing.server_exited(server_id, false).await;
	let result = reply_rx.await.expect("pending cancelled");
	assert!(
		matches!(result, Err(ref e) if e.code == xeno_lsp::ErrorCode::REQUEST_CANCELLED),
		"unexpected result: {result:?}"
	);

	let event1 = session1.recv_event().await.expect("status event");
	let event2 = session2.recv_event().await.expect("status event");
	match event1 {
		Event::LspStatus { status, .. } => {
			assert_eq!(status, xeno_broker_proto::types::LspServerStatus::Stopped);
		}
		other => panic!("unexpected event: {other:?}"),
	}
	match event2 {
		Event::LspStatus { status, .. } => {
			assert_eq!(status, xeno_broker_proto::types::LspServerStatus::Stopped);
		}
		other => panic!("unexpected event: {other:?}"),
	}
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_idle_lease_expiry_removes_server() {
	let harness = setup_routing_harness(Duration::from_secs(60)).await;
	let session1 = TestSession::new(1);

	harness
		.sessions
		.register(session1.session_id, session1.sink.clone())
		.await;

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();

	harness.routing.session_lost(session1.session_id).await;

	let (reply_tx, _reply_rx) = oneshot::channel();
	let request_id = xeno_lsp::RequestId::Number(1);
	let err = harness
		.routing
		.begin_s2c(server_id, request_id, "{\"method\":\"x\"}".into(), reply_tx)
		.await
		.unwrap_err();
	assert_eq!(err.code, xeno_lsp::ErrorCode::METHOD_NOT_FOUND);

	tokio::task::yield_now().await;
	tokio::time::advance(Duration::from_secs(61)).await;
	tokio::task::yield_now().await;

	let (reply_tx, _reply_rx) = oneshot::channel();
	let request_id = xeno_lsp::RequestId::Number(2);
	let err = harness
		.routing
		.begin_s2c(server_id, request_id, "{\"method\":\"x\"}".into(), reply_tx)
		.await
		.unwrap_err();
	assert_eq!(err.code, xeno_lsp::ErrorCode::INTERNAL_ERROR);
}

#[tokio::test(flavor = "current_thread")]
async fn test_text_sync_drop_silently_skips_forwarding() {
	let harness = setup_routing_harness(Duration::from_secs(300)).await;
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

	let config = test_config("rust-analyzer", "/project1");
	let server_id = harness
		.routing
		.lsp_start(session1.session_id, config.clone())
		.await
		.unwrap();
	let _ = harness
		.routing
		.lsp_start(session2.session_id, config)
		.await
		.unwrap();

	let did_open = xeno_lsp::AnyNotification::new(
		"textDocument/didOpen",
		serde_json::json!({
			"textDocument": { "uri": "file:///main.rs", "languageId": "rust", "version": 1, "text": "x" }
		}),
	);

	harness
		.routing
		.lsp_send_notif(
			session1.session_id,
			server_id,
			serde_json::to_string(&did_open.clone()).unwrap(),
		)
		.await
		.unwrap();

	harness
		.routing
		.lsp_send_notif(
			session2.session_id,
			server_id,
			serde_json::to_string(&did_open).unwrap(),
		)
		.await
		.unwrap();
	tokio::task::yield_now().await;

	let server = harness
		.launcher
		.get_server(server_id)
		.expect("server handle");
	let received = server.received.lock().unwrap().clone();
	let open_count = received
		.iter()
		.filter(
			|msg| matches!(msg, xeno_lsp::Message::Notification(n) if n.method == "textDocument/didOpen"),
		)
		.count();
	assert_eq!(open_count, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn test_session_send_failure_triggers_cleanup() {
	let (sessions_handle, routing_tx, sync_tx) = sessions::SessionService::start();

	let (routing_cmd_tx, mut routing_cmd_rx) = mpsc::channel(4);
	let routing_handle = routing::RoutingHandle::new(routing_cmd_tx);
	let _ = routing_tx.send(routing_handle).await;

	let (sync_cmd_tx, mut sync_cmd_rx) = mpsc::channel(4);
	let sync_handle = shared_state::SharedStateHandle::new(sync_cmd_tx);
	let _ = sync_tx.send(sync_handle).await;

	let (tx, rx) = mpsc::unbounded_channel();
	let sink = SessionSink::from_sender(tx);
	drop(rx);

	let sid = SessionId(42);
	sessions_handle.register(sid, sink).await;

	let ok = sessions_handle
		.send_checked(sid, IpcFrame::Event(Event::Heartbeat))
		.await;
	assert!(!ok);

	let routing = tokio::time::timeout(Duration::from_millis(200), routing_cmd_rx.recv())
		.await
		.ok()
		.flatten();
	match routing {
		Some(routing::RoutingCmd::SessionLost { sid: lost }) => assert_eq!(lost, sid),
		other => panic!("unexpected routing cmd: {other:?}"),
	}

	let sync = tokio::time::timeout(Duration::from_millis(200), sync_cmd_rx.recv())
		.await
		.ok()
		.flatten();
	match sync {
		Some(shared_state::SharedStateCmd::SessionLost { sid: lost }) => assert_eq!(lost, sid),
		other => panic!("unexpected sync cmd: {other:?}"),
	}
}
