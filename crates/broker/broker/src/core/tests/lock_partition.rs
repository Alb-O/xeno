//! Tests to ensure routing and sync locks are independent.

use xeno_broker_proto::types::{Event, IpcFrame};

use super::helpers::TestSession;
use crate::core::BrokerCore;

#[tokio::test(flavor = "current_thread")]
async fn test_resync_does_not_require_routing_lock() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());
	core.on_buffer_sync_open(session.session_id, "file:///test.rs", "hello", None);

	let _routing_guard = core.lock_routing_for_test();
	let resp = core.on_buffer_sync_resync(session.session_id, "file:///test.rs");
	assert!(resp.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn test_send_event_does_not_require_sync_lock() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let _sync_guard = core.lock_sync_for_test();
	let ok = core.send_event(session.session_id, IpcFrame::Event(Event::Heartbeat));
	assert!(ok);
}
