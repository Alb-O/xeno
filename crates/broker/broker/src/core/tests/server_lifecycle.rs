//! Tests for server registration, unregistration, and lifecycle.

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::BrokerCore;

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
async fn server_ids_are_sequential() {
	let core = BrokerCore::new();

	let id1 = core.next_server_id();
	let id2 = core.next_server_id();
	let id3 = core.next_server_id();

	assert_eq!(id1.0, 0);
	assert_eq!(id2.0, 1);
	assert_eq!(id3.0, 2);
}
