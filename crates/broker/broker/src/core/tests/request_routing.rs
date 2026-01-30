//! Tests for server-to-client request routing and reply handling.

use xeno_broker_proto::types::Event;

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::BrokerCore;

#[tokio::test(flavor = "current_thread")]
async fn server_to_client_request_only_leader_receives() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);
	let mut session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// Send server-to-client request
	core.send_to_leader(
		server_id,
		Event::LspRequest {
			server_id,
			message: "{\"method\":\"workspace/applyEdit\"}".to_string(),
		},
	);

	// Only leader receives
	assert!(session1.try_recv().is_some());
	assert!(session2.try_recv().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn reply_from_non_leader_is_rejected() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// Register pending request
	let req_id = xeno_lsp::RequestId::Number(1);
	let (tx, mut _rx) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	core.register_client_request(server_id, req_id.clone(), tx);

	// Try to complete from non-leader
	let result: crate::core::LspReplyResult = Ok(serde_json::Value::Null);
	let completed = core.complete_client_request(session2.session_id, server_id, req_id, result);

	assert!(!completed);
}

#[tokio::test(flavor = "current_thread")]
async fn reply_from_leader_completes_pending() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	// Register pending request
	let req_id = xeno_lsp::RequestId::Number(1);
	let (tx, mut rx) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	core.register_client_request(server_id, req_id.clone(), tx);

	// Complete from leader
	let result: crate::core::LspReplyResult = Ok(serde_json::json!({"applied": true}));
	let completed = core.complete_client_request(session1.session_id, server_id, req_id, result);

	assert!(completed);

	// Receiver should get result
	let received = rx.try_recv();
	assert!(received.is_ok());
	assert_eq!(
		received.unwrap().unwrap(),
		serde_json::json!({"applied": true})
	);
}

#[tokio::test(flavor = "current_thread")]
async fn reply_from_nonexistent_request_fails() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	// Try to complete non-existent request
	let req_id = xeno_lsp::RequestId::Number(999);
	let result: crate::core::LspReplyResult = Ok(serde_json::Value::Null);
	let completed = core.complete_client_request(session1.session_id, server_id, req_id, result);

	assert!(!completed);
}

#[tokio::test(flavor = "current_thread")]
async fn disconnect_leader_cancels_pending_requests() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);
	let session2 = TestSession::new(2);

	core.register_session(session1.session_id, session1.sink.clone());
	core.register_session(session2.session_id, session2.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);
	core.attach_session(server_id, session2.session_id);

	// Register pending request
	let req_id = xeno_lsp::RequestId::Number(1);
	let (tx, rx) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	core.register_client_request(server_id, req_id.clone(), tx);

	// Unregister leader
	core.unregister_session(session1.session_id);

	// Request should be cancelled with proper error response
	let result = rx.await.expect("oneshot should deliver a value");
	assert!(
		matches!(result, Err(ref e) if e.code == xeno_lsp::ErrorCode::REQUEST_CANCELLED),
		"Expected REQUEST_CANCELLED error, got: {:?}",
		result
	);

	// Session2 is now leader
	let req_id2 = xeno_lsp::RequestId::Number(2);
	let (tx2, _rx2) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
	let leader = core.register_client_request(server_id, req_id2, tx2);
	assert_eq!(leader, Some(session2.session_id));
}
