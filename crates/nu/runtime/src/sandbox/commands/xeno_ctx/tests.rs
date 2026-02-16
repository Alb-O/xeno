use xeno_nu_protocol::{Span, Value};

use crate::sandbox::{call_function, create_engine_state, evaluate_block, find_decl, parse_and_validate};

#[test]
fn xeno_ctx_returns_nothing_without_injection() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "xeno ctx";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(matches!(value, Value::Nothing { .. }), "xeno ctx without env should return nothing");
}

#[test]
fn xeno_ctx_returns_injected_env() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { xeno ctx }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let decl_id = find_decl(&engine_state, "go").expect("go should exist");
	let ctx = Value::string("test-ctx", Span::unknown());
	let result = call_function(&engine_state, decl_id, &[], &[("XENO_CTX", ctx)]).expect("should call");
	assert_eq!(result.as_str().unwrap(), "test-ctx");
}

#[test]
fn create_engine_state_registers_xeno_ctx_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "xeno ctx").is_some(), "xeno ctx command should be registered");
}
