use crate::sandbox::{ParsePolicy, create_engine_state, find_decl, parse_and_validate_with_policy};

#[test]
fn module_only_rejects_shadowing_xeno_call() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(
		&mut engine_state,
		"<test>",
		"export def \"xeno call\" [] { null }",
		None,
		ParsePolicy::ModuleWrapped,
	)
	.expect_err("shadowing 'xeno call' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno call"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_call_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "xeno call").is_some(), "xeno call command should be registered");
}
