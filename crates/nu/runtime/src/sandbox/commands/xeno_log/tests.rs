use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_log_passes_through_value() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | xeno log 'v'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_int().unwrap(), 42);
}

#[test]
fn module_only_rejects_shadowing_xeno_log() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(
		&mut engine_state,
		"<test>",
		r#"export def "xeno log" [] { null }"#,
		None,
		ParsePolicy::ModuleWrapped,
	)
	.expect_err("shadowing 'xeno log' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno log"), "got: {err}");
}

#[test]
fn xeno_log_unicode_truncation_does_not_panic() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let s = "\u{1F600}".repeat(80);
	let source = format!("'{s}' | xeno log 'u'");
	let parsed = parse_and_validate(&mut engine_state, "<test>", &source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_str().unwrap(), s);
}

#[test]
fn xeno_log_rejects_invalid_level() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | xeno log 'v' --level 'nope'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(result.is_err() && err_str.contains("level"), "should reject invalid level: {err_str}");
}

#[test]
fn create_engine_state_registers_xeno_log_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "xeno log").is_some(), "xeno log command should be registered");
}
