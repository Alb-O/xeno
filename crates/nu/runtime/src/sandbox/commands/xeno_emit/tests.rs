use crate::sandbox::{ParsePolicy, create_engine_state, evaluate_block, find_decl, parse_and_validate, parse_and_validate_with_policy};

#[test]
fn xeno_effect_dispatch_action_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno effect dispatch action move_right --count 3 --extend"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");
	assert_eq!(rec.get("type").unwrap().as_str().unwrap(), "dispatch");
	assert_eq!(rec.get("kind").unwrap().as_str().unwrap(), "action");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(rec.get("count").unwrap().as_int().unwrap(), 3);
	assert!(rec.get("extend").unwrap().as_bool().unwrap());
}

#[test]
fn xeno_effect_dispatch_command_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno effect dispatch command write foo.txt"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");
	assert_eq!(rec.get("type").unwrap().as_str().unwrap(), "dispatch");
	assert_eq!(rec.get("kind").unwrap().as_str().unwrap(), "command");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "write");
	let args = rec.get("args").unwrap().as_list().unwrap();
	assert_eq!(args.len(), 1);
	assert_eq!(args[0].as_str().unwrap(), "foo.txt");
}

#[test]
fn xeno_effect_dispatch_unknown_kind_errors() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno effect dispatch bogus foo"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("unknown kind should fail");
	assert!(err.contains("unknown") && err.contains("bogus"), "got: {err}");
}

#[test]
fn xeno_effect_dispatch_rejects_bad_register_len() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno effect dispatch action foo --register ab"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("multi-char register should fail");
	assert!(err.contains("one character"), "got: {err}");
}

#[test]
fn xeno_effect_dispatch_round_trips_through_decoder() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno effect dispatch action move_right --count 2 --char x"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let value = xeno_nu_data::Value::try_from(value).expect("value should convert");
	let effects = xeno_invocation::nu::decode_macro_effects(value).expect("should decode");
	assert_eq!(effects.effects.len(), 1);
	match &effects.effects[0] {
		xeno_invocation::nu::NuEffect::Dispatch(xeno_invocation::Invocation::ActionWithChar { name, count, char_arg, .. }) => {
			assert_eq!(name, "move_right");
			assert_eq!(*count, 2);
			assert_eq!(*char_arg, 'x');
		}
		other => panic!("expected Dispatch(ActionWithChar), got: {other:?}"),
	}
}

#[test]
fn xeno_effect_notify_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno effect notify warn "boom""#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");
	assert_eq!(rec.get("type").unwrap().as_str().unwrap(), "notify");
	assert_eq!(rec.get("level").unwrap().as_str().unwrap(), "warn");
	assert_eq!(rec.get("message").unwrap().as_str().unwrap(), "boom");
}

#[test]
fn legacy_xeno_emit_is_rejected() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"xeno emit action move_right --count 2 --char x"#;
	let err = parse_and_validate(&mut engine_state, "<test>", source, None).expect_err("legacy command should fail parse/compile");
	assert!(err.contains("External calls are not supported"), "got: {err}");
}

#[test]
fn module_only_rejects_shadowing_xeno_effect() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(
		&mut engine_state,
		"<test>",
		r#"export def "xeno effect" [] { null }"#,
		None,
		ParsePolicy::ModuleOnly,
	)
	.expect_err("shadowing 'xeno effect' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno effect"), "got: {err}");
}

#[test]
fn create_engine_state_registers_xeno_effect_command() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "xeno effect").is_some(), "xeno effect command should be registered");
}
