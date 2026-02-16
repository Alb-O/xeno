use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_emit_action_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno emit action move_right --count 3 --extend"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");
	assert_eq!(rec.get("kind").unwrap().as_str().unwrap(), "action");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(rec.get("count").unwrap().as_int().unwrap(), 3);
	assert_eq!(rec.get("extend").unwrap().as_bool().unwrap(), true);
}

#[test]
fn xeno_emit_command_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno emit command write foo.txt"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");
	assert_eq!(rec.get("kind").unwrap().as_str().unwrap(), "command");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "write");
	let args = rec.get("args").unwrap().as_list().unwrap();
	assert_eq!(args.len(), 1);
	assert_eq!(args[0].as_str().unwrap(), "foo.txt");
}

#[test]
fn xeno_emit_unknown_kind_errors() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno emit bogus foo"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("unknown kind should fail");
	assert!(err.contains("unknown") && err.contains("bogus"), "got: {err}");
}

#[test]
fn xeno_emit_rejects_bad_register_len() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno emit action foo --register ab"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("multi-char register should fail");
	assert!(err.contains("one character"), "got: {err}");
}

#[test]
fn xeno_emit_round_trips_through_decoder() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno emit action move_right --count 2 --char x"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let invocations = xeno_invocation::nu::decode_invocations(value).expect("should decode");
	assert_eq!(invocations.len(), 1);
	match &invocations[0] {
		xeno_invocation::Invocation::ActionWithChar { name, count, char_arg, .. } => {
			assert_eq!(name, "move_right");
			assert_eq!(*count, 2);
			assert_eq!(*char_arg, 'x');
		}
		other => panic!("expected ActionWithChar, got: {other:?}"),
	}
}

#[test]
fn module_only_rejects_shadowing_xeno_emit() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(
		&mut engine_state,
		"<test>",
		r#"export def "xeno emit" [] { null }"#,
		None,
		ParsePolicy::ModuleOnly,
	)
	.expect_err("shadowing 'xeno emit' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno emit"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_emit_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "xeno emit").is_some(), "xeno emit command should be registered");
}
