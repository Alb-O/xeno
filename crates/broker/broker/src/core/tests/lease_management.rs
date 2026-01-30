//! Tests for lease management and warm reattach functionality.

use std::time::Duration;

use super::helpers::{TestSession, mock_instance, test_config};
use crate::core::{BrokerConfig, BrokerCore};

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
	let session2 = TestSession::new(2);
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
