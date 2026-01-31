//! Tests for error handling and edge cases.

use xeno_broker_proto::types::{Event, LspServerStatus, ServerId, SessionId};

use super::helpers::{TestSession, mock_instance, test_config};
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

/// Regression test: IPC send failure triggers authoritative session cleanup.
///
/// When a session's IPC channel becomes unreachable, `handle_session_send_failure`
/// MUST unregister the session and remove it from all server attachment sets,
/// preventing dead sessions from remaining in the routing tables.
#[tokio::test(flavor = "current_thread")]
async fn session_send_failure_unregisters_session() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// Register a pending s2c request with session1 as leader/responder
	let req_id = xeno_lsp::RequestId::Number(1);
	let (tx, rx) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	let leader = core.register_client_request(server_id, req_id, tx);
	assert_eq!(leader, Some(session1.session_id));

	// Simulate IPC send failure for session1 (the leader)
	core.handle_session_send_failure(session1.session_id);

	// Session1 should be unregistered
	let (sessions, servers, _) = core.get_state();
	assert!(
		!sessions.contains(&session1.session_id),
		"Dead session should be unregistered after send failure"
	);

	// Session2 should still be registered and attached
	assert!(sessions.contains(&session2.session_id));
	let attached = &servers[&server_id];
	assert!(attached.contains(&session2.session_id));
	assert!(!attached.contains(&session1.session_id));

	// Pending request should be cancelled
	let result = rx.await.expect("oneshot should deliver a value");
	assert!(
		matches!(result, Err(ref e) if e.code == xeno_lsp::ErrorCode::REQUEST_CANCELLED),
		"Expected REQUEST_CANCELLED error, got: {result:?}"
	);
}
