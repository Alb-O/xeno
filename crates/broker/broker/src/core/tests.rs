//! Unit tests for BrokerCore.

use std::time::Duration;

use tokio::sync::mpsc;
use xeno_broker_proto::types::LspServerStatus;

use crate::core::*;

/// A test harness that captures events sent to sessions.
pub struct TestSession {
	pub session_id: SessionId,
	pub sink: SessionSink,
	pub events_rx: mpsc::UnboundedReceiver<MainLoopEvent<IpcFrame, Request, Response>>,
}

impl TestSession {
	/// Create a new test session with a unique ID.
	pub fn new(id: u64) -> Self {
		let (tx, rx) = mpsc::unbounded_channel();
		let sink = PeerSocket::from_sender(tx);
		Self {
			session_id: SessionId(id),
			sink,
			events_rx: rx,
		}
	}

	/// Try to receive an event, returning None if none available.
	pub fn try_recv(&mut self) -> Option<MainLoopEvent<IpcFrame, Request, Response>> {
		self.events_rx.try_recv().ok()
	}

	/// Wait for an event with a timeout.
	pub async fn recv_timeout(&mut self) -> Option<MainLoopEvent<IpcFrame, Request, Response>> {
		let timeout: tokio::time::Timeout<_> =
			tokio::time::timeout(Duration::from_millis(100), self.events_rx.recv());
		timeout.await.ok().flatten()
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

fn mock_instance() -> LspInstance {
	let (tx, _rx) = mpsc::unbounded_channel();
	LspInstance::mock(PeerSocket::from_sender(tx), LspServerStatus::Starting)
}

#[tokio::test(flavor = "current_thread")]
async fn project_dedup_same_config_returns_same_server_id() {
	let core = BrokerCore::new();
	let config = test_config("rust-analyzer", "/project1");

	// Not registered yet
	assert!(core.find_server_for_project(&config).is_none());

	// Register a server
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, SessionId(1));

	// Same config should return same server
	assert_eq!(core.find_server_for_project(&config), Some(server_id));

	// Different config with same project key should also match
	let config2 = test_config("rust-analyzer", "/project1");
	assert_eq!(core.find_server_for_project(&config2), Some(server_id));
}

#[tokio::test(flavor = "current_thread")]
async fn project_dedup_different_cwd_returns_different_server_id() {
	let core = BrokerCore::new();

	let config1 = test_config("rust-analyzer", "/project1");
	let config2 = test_config("rust-analyzer", "/project2");

	// Register two servers with different cwds
	let server_id1 = core.next_server_id();
	let server_id2 = core.next_server_id();

	core.register_server(server_id1, mock_instance(), &config1, SessionId(1));
	core.register_server(server_id2, mock_instance(), &config2, SessionId(2));

	// Each project should find its own server
	assert_eq!(core.find_server_for_project(&config1), Some(server_id1));
	assert_eq!(core.find_server_for_project(&config2), Some(server_id2));
}

#[tokio::test(flavor = "current_thread")]
async fn project_dedup_different_command_returns_different_server_id() {
	let core = BrokerCore::new();

	let config1 = test_config("rust-analyzer", "/project1");
	let config2 = test_config("typescript-language-server", "/project1");

	let server_id1 = core.next_server_id();
	let server_id2 = core.next_server_id();

	core.register_server(server_id1, mock_instance(), &config1, SessionId(1));
	core.register_server(server_id2, mock_instance(), &config2, SessionId(2));

	assert_eq!(core.find_server_for_project(&config1), Some(server_id1));
	assert_eq!(core.find_server_for_project(&config2), Some(server_id2));
}

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
			doc_id: DocId(1),
			uri: "file:///project1/src/main.rs".to_string(),
			version: 1,
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
	let (tx, mut rx) = tokio::sync::oneshot::channel::<crate::core::LspReplyResult>();
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

#[tokio::test(flavor = "current_thread")]
async fn unregister_server_removes_all_attachments() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session.session_id);

	assert!(core.get_server_tx(server_id).is_some());
	assert!(core.find_server_for_project(&config).is_some());

	core.unregister_server(server_id);

	assert!(core.get_server_tx(server_id).is_none());
	assert!(core.find_server_for_project(&config).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn attach_to_nonexistent_server_fails() {
	let core = BrokerCore::new();
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let attached = core.attach_session(ServerId(999), session.session_id);
	assert!(!attached);
}

#[tokio::test(flavor = "current_thread")]
async fn server_ids_are_sequential() {
	let core = BrokerCore::new();

	let id1 = core.next_server_id();
	let id2 = core.next_server_id();
	let id3 = core.next_server_id();

	assert_eq!(id1.0, 0);
	assert_eq!(id2.0, 1);
	assert_eq!(id3.0, 2);
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

fn apply_text_sync(
	core: &BrokerCore,
	session: SessionId,
	server: ServerId,
	notif: &xeno_lsp::AnyNotification,
) -> DocGateDecision {
	let decision = core.gate_text_sync(session, server, notif);
	if decision == DocGateDecision::Forward {
		core.on_editor_message(server, &xeno_lsp::Message::Notification(notif.clone()));
	}
	decision
}

fn did_open(uri: &str, version: u32) -> xeno_lsp::AnyNotification {
	xeno_lsp::AnyNotification::new(
		"textDocument/didOpen",
		serde_json::json!({
			"textDocument": { "uri": uri, "languageId": "rust", "version": version, "text": "x" }
		}),
	)
}

fn did_change(uri: &str, version: u32) -> xeno_lsp::AnyNotification {
	xeno_lsp::AnyNotification::new(
		"textDocument/didChange",
		serde_json::json!({
			"textDocument": { "uri": uri, "version": version },
			"contentChanges": [{ "text": "y" }]
		}),
	)
}

fn did_close(uri: &str) -> xeno_lsp::AnyNotification {
	xeno_lsp::AnyNotification::new(
		"textDocument/didClose",
		serde_json::json!({ "textDocument": { "uri": uri } }),
	)
}

#[tokio::test(flavor = "current_thread")]
async fn text_sync_state_machine_multi_session() {
	let core = BrokerCore::new();
	let s1 = SessionId(1);
	let s2 = SessionId(2);
	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();

	core.register_session(s1, TestSession::new(1).sink);
	core.register_session(s2, TestSession::new(2).sink);
	core.register_server(server_id, mock_instance(), &config, s1);
	core.attach_session(server_id, s2);

	let uri = "file:///main.rs";

	// 1. S1 opens: Forward
	assert_eq!(
		apply_text_sync(&core, s1, server_id, &did_open(uri, 1)),
		DocGateDecision::Forward
	);
	assert_eq!(core.get_doc_by_uri(server_id, uri).unwrap().1, 1);

	// 2. S2 opens: DropSilently
	assert_eq!(
		apply_text_sync(&core, s2, server_id, &did_open(uri, 10)),
		DocGateDecision::DropSilently
	);
	// Version should NOT change (not forwarded)
	assert_eq!(core.get_doc_by_uri(server_id, uri).unwrap().1, 1);

	// 3. S2 changes: Reject (not owner)
	assert_eq!(
		apply_text_sync(&core, s2, server_id, &did_change(uri, 11)),
		DocGateDecision::RejectNotOwner
	);

	// 4. S1 changes: Forward
	assert_eq!(
		apply_text_sync(&core, s1, server_id, &did_change(uri, 2)),
		DocGateDecision::Forward
	);
	assert_eq!(core.get_doc_by_uri(server_id, uri).unwrap().1, 2);

	// 5. S1 closes: DropSilently (S2 still has it)
	assert_eq!(
		apply_text_sync(&core, s1, server_id, &did_close(uri)),
		DocGateDecision::DropSilently
	);
	// Still registered
	assert!(core.get_doc_by_uri(server_id, uri).is_some());

	// 6. S2 changes: Forward (takeover after S1 close)
	assert_eq!(
		apply_text_sync(&core, s2, server_id, &did_change(uri, 12)),
		DocGateDecision::Forward
	);
	assert_eq!(core.get_doc_by_uri(server_id, uri).unwrap().1, 12);

	// 7. S2 closes: Forward (last close)
	assert_eq!(
		apply_text_sync(&core, s2, server_id, &did_close(uri)),
		DocGateDecision::Forward
	);
	// Should be removed from registry
	assert!(core.get_doc_by_uri(server_id, uri).is_none());
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

#[tokio::test(flavor = "current_thread")]
async fn unregister_cleans_c2s_and_docs() {
	let core = BrokerCore::new();
	let s1 = SessionId(1);
	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();

	core.register_session(s1, TestSession::new(1).sink);
	core.register_server(server_id, mock_instance(), &config, s1);

	// 1. Pending C2S
	let wire_id = xeno_lsp::RequestId::String("b:0:1".to_string());
	core.register_c2s_pending(
		server_id,
		wire_id.clone(),
		s1,
		xeno_lsp::RequestId::Number(100),
	);

	// 2. Open doc
	let uri = "file:///test.rs";
	apply_text_sync(&core, s1, server_id, &did_open(uri, 1));
	assert!(core.get_doc_by_uri(server_id, uri).is_some());

	// Unregister
	core.unregister_session(s1);

	// Assert C2S gone
	assert!(core.take_c2s_pending(server_id, &wire_id).is_none());

	// Assert doc gone
	assert!(core.get_doc_by_uri(server_id, uri).is_none());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn warm_reattach_reuses_server() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	// Detach session
	core.detach_session(server_id, session1.session_id);

	// Advance time, but less than lease (5 mins)
	tokio::time::advance(Duration::from_secs(60)).await;

	// Session 2 connects
	let mut session2 = TestSession::new(2);
	core.register_session(session2.session_id, session2.sink.clone());

	// Should find existing server and attach
	let found_id = core.find_server_for_project(&config);
	assert_eq!(found_id, Some(server_id));
	assert!(core.attach_session(server_id, session2.session_id));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn lease_expiry_terminates_server() {
	let core = BrokerCore::new_with_config(BrokerConfig {
		idle_lease: Duration::from_secs(60),
	});
	let session = TestSession::new(1);

	core.register_session(session.session_id, session.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session.session_id);

	// Detach session
	core.detach_session(server_id, session.session_id);

	// Yield to allow the lease task to be polled and register its timer
	tokio::task::yield_now().await;

	// Advance time past lease
	tokio::time::advance(Duration::from_secs(61)).await;
	// Yield again to allow the lease task to complete after wake-up
	tokio::task::yield_now().await;

	// Should be gone
	assert!(core.find_server_for_project(&config).is_none());
	assert!(core.get_server_tx(server_id).is_none());
}
