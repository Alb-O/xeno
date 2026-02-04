use std::time::Duration;

use tokio::sync::mpsc;
use xeno_broker_proto::types::{Event, IpcFrame, SessionId};

use crate::core::SessionSink;
use crate::services::{routing, sessions, shared_state};

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
