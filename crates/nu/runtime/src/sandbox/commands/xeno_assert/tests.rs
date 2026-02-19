use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_assert_passes_through_when_true() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | xeno assert true 'ok'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_int().unwrap(), 42);
}

#[test]
fn xeno_assert_fails_when_false() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | xeno assert false 'nope'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(result.is_err() && err_str.contains("xeno assert failed"), "should fail: {err_str}");
}

#[test]
fn module_only_rejects_shadowing_xeno_assert() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(
		&mut engine_state,
		"<test>",
		r#"export def "xeno assert" [] { null }"#,
		None,
		ParsePolicy::ModuleWrapped,
	)
	.expect_err("shadowing 'xeno assert' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno assert"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_assert_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "xeno assert").is_some(), "xeno assert command should be registered");
}
