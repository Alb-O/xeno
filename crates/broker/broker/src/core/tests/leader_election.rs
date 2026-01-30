//! Tests for leader election and deterministic leader selection.

use xeno_broker_proto::types::SessionId;

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::BrokerCore;

#[tokio::test(flavor = "current_thread")]
async fn first_session_becomes_leader() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	// Server-to-client request should route to session1 (leader)
	let req_id = xeno_lsp::RequestId::Number(1);
	let (tx, _rx) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	let leader = core.register_client_request(server_id, req_id, tx);

	assert_eq!(leader, Some(session1.session_id));
}

#[tokio::test(flavor = "current_thread")]
async fn detach_leader_elects_new_leader() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// First request goes to session1
	let req_id1 = xeno_lsp::RequestId::Number(1);
	let (tx1, _rx1) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	let leader1 = core.register_client_request(server_id, req_id1, tx1);
	assert_eq!(leader1, Some(session1.session_id));

	// Unregister leader
	core.unregister_session(session1.session_id);

	// New request goes to session2
	let req_id2 = xeno_lsp::RequestId::Number(2);
	let (tx2, _rx2) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	let leader2 = core.register_client_request(server_id, req_id2, tx2);
	assert_eq!(leader2, Some(session2.session_id));
}

#[tokio::test(flavor = "current_thread")]
async fn last_session_leaves_server_has_no_leader() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session.session_id);

	// First request works
	let req_id1 = xeno_lsp::RequestId::Number(1);
	let (tx1, _rx1) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	assert!(
		core.register_client_request(server_id, req_id1, tx1)
			.is_some()
	);

	// Unregister only session
	core.unregister_session(session.session_id);

	// New request returns None
	let req_id2 = xeno_lsp::RequestId::Number(2);
	let (tx2, _rx2) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	assert!(
		core.register_client_request(server_id, req_id2, tx2)
			.is_none()
	);
}

#[tokio::test(flavor = "current_thread")]
async fn deterministic_leader_election() {
	let core = BrokerCore::new();
	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();

	// Session 10 starts server (is leader)
	core.register_session(SessionId(10), TestSession::new(10).sink);
	core.register_server(server_id, mock_instance(), &config, SessionId(10));

	let (_, servers, _) = core.get_state();
	let server_attached = servers.get(&server_id).unwrap();
	assert_eq!(server_attached.len(), 1);
	// Leader check through private state via helper (requires pub for test or internal check)
	// For now we check register_client_request which uses core.leader
	let (tx, _) = tokio::sync::oneshot::channel();
	assert_eq!(
		core.register_client_request(server_id, xeno_lsp::RequestId::Number(1), tx),
		Some(SessionId(10))
	);

	// Session 7 attaches: should become new leader (min id)
	core.register_session(SessionId(7), TestSession::new(7).sink);
	core.attach_session(server_id, SessionId(7));
	let (tx, _) = tokio::sync::oneshot::channel();
	assert_eq!(
		core.register_client_request(server_id, xeno_lsp::RequestId::Number(2), tx),
		Some(SessionId(7))
	);

	// Session 2 attaches: should become new leader
	core.register_session(SessionId(2), TestSession::new(2).sink);
	core.attach_session(server_id, SessionId(2));
	let (tx, _) = tokio::sync::oneshot::channel();
	assert_eq!(
		core.register_client_request(server_id, xeno_lsp::RequestId::Number(3), tx),
		Some(SessionId(2))
	);

	// Detach session 2: leader should become 7
	core.unregister_session(SessionId(2));
	let (tx, _) = tokio::sync::oneshot::channel();
	assert_eq!(
		core.register_client_request(server_id, xeno_lsp::RequestId::Number(4), tx),
		Some(SessionId(7))
	);
}
