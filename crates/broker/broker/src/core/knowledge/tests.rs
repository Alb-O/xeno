use std::sync::Mutex;

use tempfile::TempDir;

use super::{KnowledgeCore, SCHEMA_CONFIG};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_knowledge_core_open_close() {
	let temp = TempDir::new().expect("tempdir");
	let db_path = temp.path().join("knowledge");
	let core = KnowledgeCore::open(db_path).expect("open knowledge core");
	let txn = core.storage().graph_env.read_txn().expect("read txn");
	drop(txn);
}

#[test]
fn test_schema_config_parses() {
	let result = std::panic::catch_unwind(|| {
		let _ = SCHEMA_CONFIG.clone();
	});
	assert!(result.is_ok());
}

#[test]
fn test_graceful_degradation() {
	let _guard = ENV_LOCK.lock().unwrap();
	let temp = TempDir::new().expect("tempdir");
	let bad_path = temp.path().join("state-file");
	std::fs::write(&bad_path, "not a directory").expect("write state-file");

	let old_state = std::env::var("XDG_STATE_HOME").ok();
	unsafe {
		std::env::set_var("XDG_STATE_HOME", &bad_path);
	}

	let core = super::super::BrokerCore::new_with_config(super::super::BrokerConfig::default());
	assert!(core.knowledge.is_none());

	match old_state {
		Some(value) => unsafe {
			std::env::set_var("XDG_STATE_HOME", value);
		},
		None => unsafe {
			std::env::remove_var("XDG_STATE_HOME");
		},
	}
}
