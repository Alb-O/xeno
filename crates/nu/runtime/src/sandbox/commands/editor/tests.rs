use crate::sandbox::{create_engine_state, find_decl};

#[test]
fn create_engine_state_registers_editor_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "editor").is_some(), "editor command should be registered");
}
