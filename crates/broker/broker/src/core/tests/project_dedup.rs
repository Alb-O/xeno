//! Tests for project-based server deduplication.

use xeno_broker_proto::types::SessionId;

use super::helpers::{mock_instance, test_config};
use crate::core::BrokerCore;

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
