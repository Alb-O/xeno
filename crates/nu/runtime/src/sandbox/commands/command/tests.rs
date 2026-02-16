use crate::sandbox::{create_engine_state, evaluate_block, find_decl, parse_and_validate};

#[test]
fn command_command_returns_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "command write foo.txt";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "command");
	assert_eq!(record.get("name").unwrap().as_str().unwrap(), "write");
	let args = record.get("args").unwrap().as_list().unwrap();
	assert_eq!(args.len(), 1);
	assert_eq!(args[0].as_str().unwrap(), "foo.txt");
}

#[test]
fn create_engine_state_registers_command_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "command").is_some(), "command command should be registered");
}
