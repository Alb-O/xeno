use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_effects_normalize_accepts_single_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{type: "dispatch", kind: "action", name: "move_right"} | xeno effects normalize"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 1);
	assert_eq!(list[0].as_record().unwrap().get("type").unwrap().as_str().unwrap(), "dispatch");
}

#[test]
fn xeno_effects_normalize_accepts_list_of_records() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"[{type: "dispatch", kind: "action", name: "x"}, {type: "dispatch", kind: "command", name: "y", args: ["a"]}] | xeno effects normalize"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 2);
}

#[test]
fn xeno_effects_normalize_normalizes_action_defaults() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{type: "dispatch", kind: "action", name: "x"} | xeno effects normalize"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	let rec = list[0].as_record().unwrap();
	assert_eq!(rec.get("count").unwrap().as_int().unwrap(), 1);
	assert_eq!(rec.get("extend").unwrap().as_bool().unwrap(), false);
}

#[test]
fn xeno_effects_normalize_rejects_bad_args_type() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{type: "dispatch", kind: "command", name: "x", args: [1 2]} | xeno effects normalize"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("should reject int args");
	assert!(err.contains("string"), "got: {err}");
}

#[test]
fn xeno_effects_normalize_round_trips_through_decoder() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"[{type: "dispatch", kind: "action", name: "x", count: 3}, {type: "dispatch", kind: "command", name: "y"}] | xeno effects normalize"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let effects = xeno_invocation::nu::decode_macro_effects(value).expect("should decode");
	assert_eq!(effects.effects.len(), 2);
}

#[test]
fn module_only_rejects_shadowing_xeno_effects_normalize() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"export def "xeno effects normalize" [] { null }"#;
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing 'xeno effects normalize' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno effects normalize"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_effects_normalize_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(
		find_decl(&engine_state, "xeno effects normalize").is_some(),
		"xeno effects normalize command should be registered"
	);
}
