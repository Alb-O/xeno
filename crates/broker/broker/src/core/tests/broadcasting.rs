//! Tests for event broadcasting to attached sessions.

use xeno_broker_proto::types::{DocId, Event, LspServerStatus};

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::BrokerCore;

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_broadcast_to_all_attached_sessions() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);
	let mut session3 = TestSession::new(3);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());
	core.register_session(session3.session_id, session3.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);
	core.attach_session(server_id, session3.session_id);

	// Broadcast diagnostics
	core.broadcast_to_server(
		server_id,
		Event::LspDiagnostics {
			server_id,
			doc_id: Some(DocId(1)),
			uri: "file:///project1/src/main.rs".to_string(),
			version: Some(1),
			diagnostics: "[]".to_string(),
		},
	);

	// All attached should receive
	assert!(session1.try_recv().is_some());
	assert!(session2.try_recv().is_some());
	assert!(session3.try_recv().is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn notifications_broadcast_to_all_attached_sessions() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	core.broadcast_to_server(
		server_id,
		Event::LspMessage {
			server_id,
			message: "{\"method\":\"$/progress\"}".to_string(),
		},
	);

	assert!(session1.try_recv().is_some());
	assert!(session2.try_recv().is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn status_broadcast_on_state_change() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// Clear initial messages
	while session1.try_recv().is_some() {}
	while session2.try_recv().is_some() {}

	// Change status
	core.set_server_status(server_id, LspServerStatus::Running);

	assert!(session1.try_recv().is_some());
	assert!(session2.try_recv().is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn status_change_only_broadcasts_on_actual_change() {
	let core = BrokerCore::new();
	let mut session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session.session_id);

	// Clear any initial messages
	while session.try_recv().is_some() {}

	// Set to Running
	core.set_server_status(server_id, LspServerStatus::Running);
	assert!(session.try_recv().is_some());

	// Set to Running again - should NOT broadcast
	core.set_server_status(server_id, LspServerStatus::Running);
	assert!(session.try_recv().is_none());

	// Change to Stopped - should broadcast
	core.set_server_status(server_id, LspServerStatus::Stopped);
	assert!(session.try_recv().is_some());
}
