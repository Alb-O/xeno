use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_emit_many_accepts_single_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{kind: "action", name: "move_right"} | xeno emit-many"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 1);
	assert_eq!(list[0].as_record().unwrap().get("kind").unwrap().as_str().unwrap(), "action");
}

#[test]
fn xeno_emit_many_accepts_list_of_records() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"[{kind: "action", name: "x"}, {kind: "command", name: "y", args: ["a"]}] | xeno emit-many"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 2);
}

#[test]
fn xeno_emit_many_normalizes_action_defaults() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{kind: "action", name: "x"} | xeno emit-many"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	let rec = list[0].as_record().unwrap();
	assert_eq!(rec.get("count").unwrap().as_int().unwrap(), 1);
	assert_eq!(rec.get("extend").unwrap().as_bool().unwrap(), false);
}

#[test]
fn xeno_emit_many_rejects_bad_args_type() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{kind: "command", name: "x", args: [1 2]} | xeno emit-many"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("should reject int args");
	assert!(err.contains("string"), "got: {err}");
}

#[test]
fn xeno_emit_many_round_trips_through_decoder() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"[{kind: "action", name: "x", count: 3}, {kind: "command", name: "y"}] | xeno emit-many"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let invocations = xeno_invocation::nu::decode_invocations(value).expect("should decode");
	assert_eq!(invocations.len(), 2);
}

#[test]
fn module_only_rejects_shadowing_xeno_emit_many() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"export def "xeno emit-many" [] { null }"#;
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing 'xeno emit-many' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno emit-many"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_emit_many_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(
		find_decl(&engine_state, "xeno emit-many").is_some(),
		"xeno emit-many command should be registered"
	);
}
