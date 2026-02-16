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
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "42", None, ParsePolicy::ModuleOnly).expect_err("bare expression should be rejected");
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
	parse_and_validate_with_policy(&mut engine_state, "<test>", "42", None, ParsePolicy::Script).expect("bare expression should be allowed in Script policy");
}

#[test]
fn action_command_returns_custom_value() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "action move_right --count 2";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

	// Should be a custom value (InvocationValue)
	let cv = value.as_custom_value().expect("should be custom value");
	let iv = cv
		.as_any()
		.downcast_ref::<xeno_invocation::nu::InvocationValue>()
		.expect("should downcast to InvocationValue");
	assert!(matches!(
		&iv.0,
		xeno_invocation::Invocation::Action { name, count: 2, extend: false, register: None } if name == "move_right"
	));
}

#[test]
fn command_command_returns_custom_value() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "command write foo.txt";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

	let cv = value.as_custom_value().expect("should be custom value");
	let iv = cv
		.as_any()
		.downcast_ref::<xeno_invocation::nu::InvocationValue>()
		.expect("should downcast to InvocationValue");
	assert!(matches!(
		&iv.0,
		xeno_invocation::Invocation::Command { name, args } if name == "write" && args == &["foo.txt"]
	));
}

#[test]
fn commands_available_in_module_only_scripts() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { action move_right --count 5 }";
	parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
		.expect("commands should be available in module-only scripts");
	let decl_id = find_decl(&engine_state, "go").expect("go should exist");
	let value = call_function(&engine_state, decl_id, &[], &[]).expect("should call");
	let cv = value.as_custom_value().expect("should be custom value");
	let iv = cv.as_any().downcast_ref::<xeno_invocation::nu::InvocationValue>().expect("should downcast");
	assert!(matches!(
		&iv.0,
		xeno_invocation::Invocation::Action { name, count: 5, .. } if name == "move_right"
	));
}

#[test]
fn create_engine_state_registers_builtin_commands() {
	let engine_state = create_engine_state(None).expect("engine state should be created");
	assert!(find_decl(&engine_state, "action").is_some(), "action command should be registered");
	assert!(find_decl(&engine_state, "command").is_some(), "command command should be registered");
	assert!(find_decl(&engine_state, "editor").is_some(), "editor command should be registered");
	assert!(find_decl(&engine_state, "nu run").is_some(), "nu run command should be registered");
	assert!(find_decl(&engine_state, "xeno ctx").is_some(), "xeno ctx command should be registered");
}

#[test]
fn module_only_rejects_shadowing_action_command() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "export def action [] { null }", None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing action should be rejected");
	assert!(err.contains("reserved") && err.contains("action"), "got: {err}");
}

#[test]
fn module_only_rejects_shadowing_multiword_command() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "export def \"nu run\" [] { null }", None, ParsePolicy::ModuleOnly)
		.expect_err("shadowing 'nu run' should be rejected");
	assert!(err.contains("reserved") && err.contains("nu run"), "got: {err}");
}

#[test]
fn xeno_ctx_returns_nothing_without_injection() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "xeno ctx";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(matches!(value, Value::Nothing { .. }), "xeno ctx without env should return nothing");
}

#[test]
fn docs_xeno_nu_example_parses_under_module_only() {
	let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/xeno.nu.example");
	let source = std::fs::read_to_string(&example_path).expect("docs/xeno.nu.example should exist");
	let mut engine_state = create_engine_state(None).expect("engine state");
	parse_and_validate_with_policy(&mut engine_state, "docs/xeno.nu.example", &source, None, ParsePolicy::ModuleOnly)
		.expect("docs example should parse under ModuleOnly policy");
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
