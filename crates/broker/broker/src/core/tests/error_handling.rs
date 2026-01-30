//! Tests for error handling and edge cases.

use xeno_broker_proto::types::{Event, LspServerStatus, ServerId, SessionId};

use super::helpers::TestSession;
use crate::core::{BrokerCore, IpcFrame};

#[tokio::test(flavor = "current_thread")]
async fn attach_to_nonexistent_server_fails() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let attached = core.attach_session(ServerId(999), session.session_id);
	assert!(!attached);
}

#[tokio::test(flavor = "current_thread")]
async fn send_event_to_unregistered_session_is_noop() {
	let core = BrokerCore::new();

	let frame = IpcFrame::Event(Event::Heartbeat);
	core.send_event(SessionId(999), frame);
	// Should not panic
}

#[tokio::test(flavor = "current_thread")]
async fn broadcast_to_nonexistent_server_is_noop() {
	let core = BrokerCore::new();

	core.broadcast_to_server(
		ServerId(999),
		Event::LspStatus {
			server_id: ServerId(999),
			status: LspServerStatus::Running,
		},
	);
	// Should not panic
}

#[tokio::test(flavor = "current_thread")]
async fn send_to_leader_of_nonexistent_server_is_noop() {
	let core = BrokerCore::new();

	core.send_to_leader(
		ServerId(999),
		Event::LspRequest {
			server_id: ServerId(999),
			message: "{}".to_string(),
		},
	);
	// Should not panic
}
