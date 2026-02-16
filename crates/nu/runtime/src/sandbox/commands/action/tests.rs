use xeno_invocation::schema;

use crate::sandbox::{ParsePolicy, call_function, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn action_command_returns_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "action move_right --count 2";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "action");
	assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(record.get("count").unwrap().as_int().unwrap(), 2);
}

#[test]
fn commands_available_in_module_only_scripts() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { action move_right --count 5 }";
	parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
		.expect("commands should be available in module-only scripts");
	let decl_id = find_decl(&engine_state, "go").expect("go should exist");
	let value = call_function(&engine_state, decl_id, &[], &[]).expect("should call");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "action");
	assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(record.get("count").unwrap().as_int().unwrap(), 5);
}

#[test]
fn module_only_rejects_shadowing_action_command() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "export def action [] { null }", None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing action should be rejected");
	assert!(err.contains("reserved") && err.contains("action"), "got: {err}");
}

#[test]
fn action_command_uses_schema_constants() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"action move_right --count 2 --extend --register r --char x"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");

	assert_eq!(rec.get(schema::KIND).unwrap().as_str().unwrap(), schema::KIND_ACTION);
	assert_eq!(rec.get(schema::NAME).unwrap().as_str().unwrap(), "move_right");
	assert_eq!(rec.get(schema::COUNT).unwrap().as_int().unwrap(), 2);
	assert_eq!(rec.get(schema::EXTEND).unwrap().as_bool().unwrap(), true);
	let reg = rec.get(schema::REGISTER).unwrap().as_str().unwrap();
	assert_eq!(reg.len(), 1, "register should be single char");
	assert_eq!(reg, "r");
	let ch = rec.get(schema::CHAR).unwrap().as_str().unwrap();
	assert_eq!(ch.len(), 1, "char should be single char");
	assert_eq!(ch, "x");
}

#[test]
fn create_engine_state_registers_action_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "action").is_some(), "action command should be registered");
}
