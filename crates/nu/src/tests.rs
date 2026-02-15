use super::*;

#[test]
fn call_function_with_args_and_env() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def greet [name: string] { $\"hello ($name) ($env.XENO_CTX)\" }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

	let decl_id = find_decl(&engine_state, "greet").expect("greet should be registered");
	assert!(parsed.script_decl_ids.contains(&decl_id), "greet should be in script_decl_ids");
	let ctx_val = Value::string("ctx-value", Span::unknown());
	let result = call_function(&engine_state, decl_id, &["world".to_string()], &[("XENO_CTX", ctx_val)]).expect("call should succeed");
	assert_eq!(result.as_str().unwrap(), "hello world ctx-value");
}

#[test]
fn call_function_does_not_mutate_engine_state() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def echo-it [x: string] { $x }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

	let num_blocks_before = engine_state.num_blocks();
	let decl_id = find_decl(&engine_state, "echo-it").expect("echo-it should be registered");

	for _ in 0..10 {
		let _ = call_function(&engine_state, decl_id, &["hi".to_string()], &[]).expect("call should succeed");
	}

	assert_eq!(engine_state.num_blocks(), num_blocks_before, "engine state should not accumulate blocks");
}

#[test]
fn script_decl_ids_excludes_builtins() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def my-func [] { 42 }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

	// "if" is a builtin — it should not be in script_decl_ids
	let if_decl = find_decl(&engine_state, "if").expect("if should exist");
	assert!(!parsed.script_decl_ids.contains(&if_decl), "builtins must not appear in script_decl_ids");

	// "my-func" should be in script_decl_ids
	let my_func = find_decl(&engine_state, "my-func").expect("my-func should exist");
	assert!(parsed.script_decl_ids.contains(&my_func), "script defs must appear in script_decl_ids");
}

#[test]
fn parse_and_validate_registers_defs_without_eval() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { 1 }";
	let _parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	// No evaluate_block — defs should still be registered by parse+merge.
	assert!(find_decl(&engine_state, "go").is_some(), "go should be registered without evaluation");
}

#[test]
fn call_function_supports_alias_decls() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export alias go = echo editor:stats";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

	let decl_id = find_decl(&engine_state, "go").expect("go should be registered");
	assert!(parsed.script_decl_ids.contains(&decl_id), "go alias should be in script_decl_ids");

	let result = call_function(&engine_state, decl_id, &[], &[]).expect("alias call should succeed");
	assert_eq!(result.as_str().unwrap(), "editor:stats");
}

#[test]
fn call_function_owned_supports_alias_decls() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export alias go = echo editor:stats";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

	let decl_id = find_decl(&engine_state, "go").expect("go should be registered");
	assert!(parsed.script_decl_ids.contains(&decl_id), "go alias should be in script_decl_ids");

	let result = call_function_owned(&engine_state, decl_id, vec![], vec![]).expect("alias call should succeed");
	assert_eq!(result.as_str().unwrap(), "editor:stats");
}

#[test]
fn recursive_function_hits_recursion_limit() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def recur [] { recur }\nrecur";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("recursive script should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("recursive script must error");
	let msg = err.to_ascii_lowercase();
	assert!(msg.contains("recursion") || msg.contains("stack") || msg.contains("overflow"), "{err}");
}

#[test]
fn module_only_accepts_export_def() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { 1 }";
	parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly).expect("export def should be allowed");
}

#[test]
fn module_only_accepts_use_and_const() {
	let temp = tempfile::tempdir().expect("temp dir");
	std::fs::write(temp.path().join("helper.nu"), "export def x [] { 1 }").unwrap();
	let mut engine_state = create_engine_state(Some(temp.path())).expect("engine state");
	let source = "const A = 42\nuse helper.nu *\nexport def go [] { x }";
	parse_and_validate_with_policy(&mut engine_state, "<test>", source, Some(temp.path()), ParsePolicy::ModuleOnly)
		.expect("const + use + export def should be allowed");
}

#[test]
fn module_only_rejects_expression() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err =
		parse_and_validate_with_policy(&mut engine_state, "<test>", "42", None, ParsePolicy::ModuleOnly).expect_err("bare expression should be rejected");
	assert!(err.contains("module-only"), "{err}");
}

#[test]
fn module_only_rejects_let() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "let x = 1", None, ParsePolicy::ModuleOnly)
		.expect_err("let should be rejected in module-only");
	assert!(err.contains("module-only") && err.contains("let"), "{err}");
}

#[test]
fn module_only_rejects_mut() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "mut x = 1", None, ParsePolicy::ModuleOnly)
		.expect_err("mut should be rejected in module-only");
	assert!(err.contains("module-only") && err.contains("mut"), "{err}");
}

#[test]
fn script_policy_allows_expressions() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	parse_and_validate_with_policy(&mut engine_state, "<test>", "42", None, ParsePolicy::Script)
		.expect("bare expression should be allowed in Script policy");
}

#[test]
fn prelude_action_constructor_returns_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "action move_right --count 2";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "action");
	assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(record.get("count").unwrap().as_int().unwrap(), 2);
	assert!(record.get("register").unwrap().is_nothing());
	assert!(record.get("char").unwrap().is_nothing());
}

#[test]
fn prelude_command_constructor_returns_record() {
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
fn prelude_str_helpers_work() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#""hello world" | str ends-with "world""#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value, Value::test_bool(true));
}

#[test]
fn prelude_available_in_module_only_scripts() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { action move_right --count 5 }";
	let parsed = parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
		.expect("prelude should be available in module-only scripts");
	let decl_id = find_decl(&engine_state, "go").expect("go should exist");
	let value = call_function(&engine_state, decl_id, &[], &[]).expect("should call");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(record.get("count").unwrap().as_int().unwrap(), 5);
}

#[test]
fn str_shim_defs_work_in_sandbox() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"export def "str ends-with" [suffix: string] { $in ends-with $suffix }
"abc" | str ends-with "bc""#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("str shim should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("str shim should evaluate");
	assert_eq!(value, nu_protocol::Value::test_bool(true));
}

#[test]
fn prelude_default_replaces_null() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"null | default "x""#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value, nu_protocol::Value::test_string("x"));
}

#[test]
fn prelude_default_preserves_non_null() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#""y" | default "x""#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value, nu_protocol::Value::test_string("y"));
}

#[test]
fn prelude_is_null() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let parsed = parse_and_validate(&mut engine_state, "<test>", "null | is-null", None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value, nu_protocol::Value::test_bool(true));

	let mut engine_state = create_engine_state(None).expect("engine state");
	let parsed = parse_and_validate(&mut engine_state, "<test>", r#""hi" | is-null"#, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value, nu_protocol::Value::test_bool(false));
}

#[test]
fn create_engine_state_succeeds_and_exposes_prelude_version() {
	let mut engine_state = create_engine_state(None).expect("engine state should be created");
	let parsed = parse_and_validate(&mut engine_state, "<test>", "$XENO_PRELUDE_VERSION", None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value, nu_protocol::Value::test_int(XENO_PRELUDE_VERSION));
}
