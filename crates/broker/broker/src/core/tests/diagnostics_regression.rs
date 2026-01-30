//! Regression tests for diagnostics broadcasting edge cases.

use xeno_broker_proto::types::Event;
use xeno_rpc::MainLoopEvent;

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::{BrokerCore, IpcFrame};

/// Regression test: Diagnostics should broadcast even when URI is not yet tracked.
///
/// This tests the "startup race" fix where publishDiagnostics can arrive
/// before didOpen registers the document in the broker's doc registry.
#[tokio::test(flavor = "current_thread")]
async fn diagnostics_broadcast_before_doc_registered() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	let uri = "file:///project1/src/new_file.rs";

	// Verify document is NOT yet registered
	assert!(core.get_doc_by_uri(server_id, uri).is_none());

	// Broadcast diagnostics BEFORE didOpen
	core.broadcast_to_server(
		server_id,
		Event::LspDiagnostics {
			server_id,
			doc_id: None, // No doc_id because not registered
			uri: uri.to_string(),
			version: Some(1), // Version from LSP payload
			diagnostics: "[{\"message\":\"unused import\"}]".to_string(),
		},
	);

	// Session should still receive the diagnostics
	let received = session1.try_recv();
	assert!(received.is_some());
	if let Some(MainLoopEvent::Outgoing(IpcFrame::Event(Event::LspDiagnostics {
		uri: recv_uri,
		..
	}))) = received
	{
		assert_eq!(recv_uri, uri);
	} else {
		panic!("Expected LspDiagnostics event");
	}
}

/// Regression test: Diagnostics should broadcast even after all sessions close a document.
///
/// This tests that diagnostics are not dropped when the broker removes the document
/// from its tracking registry (because all sessions closed it), but the LSP server
/// still sends diagnostics for that URI.
#[tokio::test(flavor = "current_thread")]
async fn diagnostics_broadcast_after_doc_closed_by_all_sessions() {
	let core = BrokerCore::new();
	let session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	let uri = "file:///project1/src/closed.rs";

	// Open the document via gate_text_sync (which tracks ownership)
	let did_open = xeno_lsp::AnyNotification::new(
		"textDocument/didOpen",
		serde_json::json!({
			"textDocument": {
				"uri": uri,
				"version": 1
			}
		}),
	);
	let _ = core.gate_text_sync(session1.session_id, server_id, &did_open);

	// Also call on_editor_message to register the doc in the tracking registry
	core.on_editor_message(server_id, &xeno_lsp::Message::Notification(did_open));

	// Verify doc is tracked
	assert!(core.get_doc_by_uri(server_id, uri).is_some());

	// Unregister the session (closes all docs)
	core.unregister_session(session1.session_id);

	// Re-register session to receive events
	let mut session2 = TestSession::new(2);
	core.register_session(session2.session_id, session2.sink.clone());
	core.attach_session(server_id, session2.session_id);

	// Verify doc is NO LONGER tracked (removed because no sessions have it open)
	assert!(core.get_doc_by_uri(server_id, uri).is_none());

	// Server sends diagnostics for the closed file
	core.broadcast_to_server(
		server_id,
		Event::LspDiagnostics {
			server_id,
			doc_id: None, // No doc_id because not tracked
			uri: uri.to_string(),
			version: Some(5), // Version from LSP payload
			diagnostics: "[]".to_string(),
		},
	);

	// Session should still receive the diagnostics
	let received = session2.try_recv();
	assert!(received.is_some());
	if let Some(MainLoopEvent::Outgoing(IpcFrame::Event(Event::LspDiagnostics {
		uri: recv_uri,
		..
	}))) = received
	{
		assert_eq!(recv_uri, uri);
	} else {
		panic!("Expected LspDiagnostics event");
	}
}

/// Regression test: Diagnostics version should come from LSP payload, not broker tracking.
///
/// This verifies that when the LSP server includes a version in publishDiagnostics,
/// that version is used instead of the broker's internal document version tracking.
#[tokio::test(flavor = "current_thread")]
async fn diagnostics_use_lsp_payload_version_not_broker_version() {
	let core = BrokerCore::new();
	let mut session1 = TestSession::new(1);

	core.register_session(session1.session_id, session1.sink.clone());

	let config = test_config("rust-analyzer", "/project1");
	let server_id = core.next_server_id();
	core.register_server(server_id, mock_instance(), &config, session1.session_id);

	let uri = "file:///project1/src/version_test.rs";

	// Register doc with broker version 10
	let did_change = xeno_lsp::AnyNotification::new(
		"textDocument/didChange",
		serde_json::json!({
			"textDocument": {
				"uri": uri,
				"version": 10
			}
		}),
	);
	core.on_editor_message(server_id, &xeno_lsp::Message::Notification(did_change));

	// Verify broker has version 10
	assert_eq!(core.get_doc_by_uri(server_id, uri).unwrap().1, 10);

	// Broadcast diagnostics with LSP payload version 7 (different from broker's 10)
	core.broadcast_to_server(
		server_id,
		Event::LspDiagnostics {
			server_id,
			doc_id: core.get_doc_by_uri(server_id, uri).map(|(id, _)| id),
			uri: uri.to_string(),
			version: Some(7), // LSP payload version takes precedence
			diagnostics: "[]".to_string(),
		},
	);

	// Session should receive diagnostics with version 7 (from payload, not broker's 10)
	let received = session1.try_recv();
	assert!(received.is_some());
	if let Some(MainLoopEvent::Outgoing(IpcFrame::Event(Event::LspDiagnostics {
		version: recv_version,
		..
	}))) = received
	{
		assert_eq!(recv_version, Some(7));
	} else {
		panic!("Expected LspDiagnostics event with version");
	}
}
