//! Tests for text synchronization state machine and document ownership.

use xeno_broker_proto::types::SessionId;

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::{BrokerCore, DocGateDecision};

fn apply_text_sync(
	core: &BrokerCore,
	session: SessionId,
	server: xeno_broker_proto::types::ServerId,
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
