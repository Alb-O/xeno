use crate::sandbox::{ParsePolicy, create_engine_state, find_decl, parse_and_validate_with_policy};

#[test]
fn module_only_rejects_shadowing_multiword_command() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "export def \"nu run\" [] { null }", None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing 'nu run' should be rejected");
	assert!(err.contains("reserved") && err.contains("nu run"), "got: {err}");
}

#[test]
fn create_engine_state_registers_nu_run_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "nu run").is_some(), "nu run command should be registered");
}
