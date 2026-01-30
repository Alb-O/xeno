//! Tests for session attach/detach and lifecycle management.

use xeno_broker_proto::types::{Event, LspServerStatus};

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::BrokerCore;

#[tokio::test(flavor = "current_thread")]
async fn session_attach_updates_both_directions() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	// Attach second session
	let mut session2 = TestSession::new(2);
	core.register_session(session2.session_id, session2.sink.clone());
	assert!(core.attach_session(server_id, session2.session_id));

	// Broadcast to both
	core.broadcast_to_server(
		server_id,
		Event::LspStatus {
			server_id,
			status: LspServerStatus::Running,
		},
	);

	// Both should receive
	assert!(session1.try_recv().is_some());
	assert!(session2.try_recv().is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn session_detach_cleans_both_directions() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// Unregister session1
	core.unregister_session(session1.session_id);

	// Broadcasting should still reach session2
	core.broadcast_to_server(
		server_id,
		Event::LspStatus {
			server_id,
			status: LspServerStatus::Running,
		},
	);

	assert!(session1.try_recv().is_none());
	assert!(session2.try_recv().is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn disconnect_session_cleans_all_attachments() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let config1 = test_config("rust-analyzer", "/project1");
	let config2 = test_config("typescript-language-server", "/project2");

	let server_id1 = core.next_server_id();
	let server_id2 = core.next_server_id();

	core.register_server(server_id1, mock_instance(), &config1, session.session_id);
	core.register_server(server_id2, mock_instance(), &config2, session.session_id);

	// Unregister session
	core.unregister_session(session.session_id);

	// Servers should still exist but session is detached
	assert!(core.get_server_tx(server_id1).is_some());
	assert!(core.get_server_tx(server_id2).is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn multiple_servers_per_session_isolation() {
	let core = BrokerCore::new();
	let mut session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let config1 = test_config("rust-analyzer", "/project1");
	let config2 = test_config("typescript-language-server", "/project2");

	let server_id1 = core.next_server_id();
	let server_id2 = core.next_server_id();

	core.register_server(server_id1, mock_instance(), &config1, session.session_id);
	core.register_server(server_id2, mock_instance(), &config2, session.session_id);

	// Broadcast to server1 only
	core.broadcast_to_server(
		server_id1,
		Event::LspStatus {
			server_id: server_id1,
			status: LspServerStatus::Running,
		},
	);

	// Should receive exactly one event
	assert!(session.try_recv().is_some());
	assert!(session.try_recv().is_none());
}
