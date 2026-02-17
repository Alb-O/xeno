use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_is_effect_true_for_valid() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{type: "dispatch", kind: "action", name: "x"} | xeno is-effect"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(value.as_bool().unwrap());
}

#[test]
fn xeno_is_effect_false_for_non_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"42 | xeno is-effect"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(!value.as_bool().unwrap());
}

#[test]
fn module_only_rejects_shadowing_xeno_is_effect() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"export def "xeno is-effect" [] { null }"#;
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing 'xeno is-effect' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno is-effect"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_is_effect_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(
		find_decl(&engine_state, "xeno is-effect").is_some(),
		"xeno is-effect command should be registered"
	);
}
