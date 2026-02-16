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
	let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/xeno.nu.example");
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

// ---------------------------------------------------------------------------
// Range ban
// ---------------------------------------------------------------------------

#[test]
fn range_expression_is_rejected() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate(&mut engine_state, "<test>", "1..10", None).expect_err("range should be rejected");
	assert!(err.contains("range") && err.contains("disabled"), "got: {err}");
}

#[test]
fn range_in_function_body_is_rejected() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate(&mut engine_state, "<test>", "export def go [] { 1..10 }", None).expect_err("range in function should be rejected");
	assert!(err.contains("range") && err.contains("disabled"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Input caps
// ---------------------------------------------------------------------------

#[test]
fn call_rejects_too_many_args() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def echo-it [x: string] { $x }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let decl_id = find_decl(&engine_state, "echo-it").expect("echo-it should exist");

	let args: Vec<String> = (0..100).map(|i| format!("arg{i}")).collect();
	let err = call_function(&engine_state, decl_id, &args, &[]).expect_err("too many args should be rejected");
	assert!(err.contains("exceeds limit"), "got: {err}");
}

#[test]
fn call_rejects_overlong_arg() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def echo-it [x: string] { $x }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let decl_id = find_decl(&engine_state, "echo-it").expect("echo-it should exist");

	let err = call_function(&engine_state, decl_id, &["x".repeat(5000)], &[]).expect_err("overlong arg should be rejected");
	assert!(err.contains("exceeds limit"), "got: {err}");
}

#[test]
fn call_rejects_oversized_env_value() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "export def go [] { xeno ctx }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let decl_id = find_decl(&engine_state, "go").expect("go should exist");

	let big = Value::string("x".repeat(5000), Span::unknown());
	let err = call_function(&engine_state, decl_id, &[], &[("XENO_CTX", big)]).expect_err("oversized env should be rejected");
	assert!(err.contains("exceeds limit"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Safe stdlib commands
// ---------------------------------------------------------------------------

#[test]
fn safe_stdlib_each_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 2 3] | each {|e| $e * 2 }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 3);
	assert_eq!(list[0].as_int().unwrap(), 2);
	assert_eq!(list[1].as_int().unwrap(), 4);
	assert_eq!(list[2].as_int().unwrap(), 6);
}

#[test]
fn safe_stdlib_reduce_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 2 3 4] | reduce {|it, acc| $it + $acc }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_int().unwrap(), 10);
}

#[test]
fn safe_stdlib_length_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 2 3] | length";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_int().unwrap(), 3);
}

#[test]
fn safe_stdlib_is_empty_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[] | is-empty";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(value.as_bool().unwrap());
}

#[test]
fn safe_stdlib_where_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 2 3 4 5] | where $it > 3";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 2);
	assert_eq!(list[0].as_int().unwrap(), 4);
	assert_eq!(list[1].as_int().unwrap(), 5);
}

#[test]
fn safe_stdlib_get_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "{name: 'test', value: 42} | get name";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_str().unwrap(), "test");
}

#[test]
fn safe_stdlib_str_contains_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'hello world' | str contains 'world'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(value.as_bool().unwrap());
}

#[test]
fn safe_stdlib_str_trim_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "' hello ' | str trim";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_str().unwrap(), "hello");
}

#[test]
fn safe_stdlib_split_row_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'a--b--c' | split row '--'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 3);
	assert_eq!(list[0].as_str().unwrap(), "a");
	assert_eq!(list[1].as_str().unwrap(), "b");
	assert_eq!(list[2].as_str().unwrap(), "c");
}

#[test]
fn safe_stdlib_split_row_regex_disabled() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'a b c' | split row -r '\\s+'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("regex should be rejected");
	assert!(err.contains("disabled") || err.contains("not available"), "got: {err}");
}

#[test]
fn safe_stdlib_str_replace_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'hello world' | str replace 'world' 'nu'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_str().unwrap(), "hello nu");
}

#[test]
fn safe_stdlib_str_replace_all_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'abc abc' | str replace --all 'b' 'z'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_str().unwrap(), "azc azc");
}

#[test]
fn safe_stdlib_select_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "{a: 1, b: 2, c: 3} | select a c";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("a").unwrap().as_int().unwrap(), 1);
	assert_eq!(record.get("c").unwrap().as_int().unwrap(), 3);
	assert!(record.get("b").is_none());
}

#[test]
fn safe_stdlib_select_table_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[name age]; ['Alice' 30] ['Bob' 25]] | select name";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 2);
	let rec0 = list[0].as_record().expect("row 0 should be record");
	assert_eq!(rec0.get("name").unwrap().as_str().unwrap(), "Alice");
	assert!(rec0.get("age").is_none());
}

#[test]
fn safe_stdlib_forbidden_commands_missing() {
	let engine_state = create_engine_state(None).expect("engine state");
	assert!(find_decl(&engine_state, "ls").is_none(), "ls should not be registered");
	assert!(find_decl(&engine_state, "open").is_none(), "open should not be registered");
	assert!(find_decl(&engine_state, "http get").is_none(), "http get should not be registered");
}

#[test]
fn safe_stdlib_each_rejects_too_many_items() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	// Build a list literal exceeding MAX_ITEMS (10000)
	let items: String = (0..10001).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
	let source = format!("[{items}] | each {{|e| $e }}");
	let parsed = parse_and_validate(&mut engine_state, "<test>", &source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(
		err_str.contains("sandbox limit exceeded") || err_str.contains("exceeds"),
		"should hit limit: {err_str}"
	);
}

#[test]
fn safe_stdlib_split_row_rejects_too_many_segments() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	// Build a string with 10001 'a' separated by ','
	let big = format!("'{}' | split row ','", "a,".repeat(10001));
	let parsed = parse_and_validate(&mut engine_state, "<test>", &big, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(
		err_str.contains("sandbox limit exceeded") || err_str.contains("exceeds"),
		"should hit limit: {err_str}"
	);
}

#[test]
fn safe_stdlib_select_rejects_too_many_columns() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	// Build select with 129 columns
	let cols: Vec<String> = (0..129).map(|i| format!("c{i}")).collect();
	let record_fields: String = cols.iter().map(|c| format!("{c}: 1")).collect::<Vec<_>>().join(", ");
	let select_cols: String = cols.join(" ");
	let source = format!("{{ {record_fields} }} | select {select_cols}");
	let parsed = parse_and_validate(&mut engine_state, "<test>", &source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(
		err_str.contains("sandbox limit exceeded") || err_str.contains("exceeds"),
		"should hit limit: {err_str}"
	);
}

#[test]
fn str_trim_column_table_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[name]; [' a ']] | str trim name";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	let rec = list[0].as_record().expect("should be record");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "a");
}

#[test]
fn str_replace_column_table_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[name]; ['a a']] | str replace --all 'a' 'b' name";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	let rec = list[0].as_record().expect("should be record");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "b b");
}

#[test]
fn split_row_column_table_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[s]; ['a--b']] | split row '--' s";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	let rec = list[0].as_record().expect("should be record");
	let parts = rec.get("s").unwrap().as_list().expect("should be list");
	assert_eq!(parts.len(), 2);
	assert_eq!(parts[0].as_str().unwrap(), "a");
	assert_eq!(parts[1].as_str().unwrap(), "b");
}

#[test]
fn str_trim_column_record_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "{name: ' a '} | str trim name";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let rec = value.as_record().expect("should be record");
	assert_eq!(rec.get("name").unwrap().as_str().unwrap(), "a");
}

#[test]
fn string_command_rejects_non_record_in_column_mode() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'hello' | str trim name";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	assert!(result.is_err(), "should error on non-record input in column mode");
}

#[test]
fn string_command_rejects_complex_cell_path() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[a]; [{b: ' x '}]] | str trim a.b";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(
		err_str.contains("complex cell paths disabled") || err_str.contains("Complex cell paths"),
		"should reject complex path: {err_str}"
	);
}

#[test]
fn into_int_scalar_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'42' | into int";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_int().unwrap(), 42);
}

#[test]
fn into_int_list_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "['1' '2'] | each {|e| $e | into int }";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_int().unwrap(), 1);
	assert_eq!(list[1].as_int().unwrap(), 2);
}

#[test]
fn into_bool_scalar_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'true' | into bool";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert!(value.as_bool().unwrap());
}

#[test]
fn into_string_scalar_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | into string";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_str().unwrap(), "42");
}

#[test]
fn into_int_column_table_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[n]; ['1'] ['2']] | into int n";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_record().unwrap().get("n").unwrap().as_int().unwrap(), 1);
	assert_eq!(list[1].as_record().unwrap().get("n").unwrap().as_int().unwrap(), 2);
}

#[test]
fn into_int_rejects_bad_parse() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "'nope' | into int";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	assert!(result.is_err(), "should error on bad parse");
}

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
		ParsePolicy::ModuleOnly,
	)
	.expect_err("shadowing 'xeno log' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno log"), "got: {err}");
}

#[test]
fn xeno_log_unicode_truncation_does_not_panic() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let s = "😀".repeat(80); // 80*4=320 bytes, crosses MAX_LOG_STRING=200
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
fn safe_stdlib_append_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 2] | append 3";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 3);
	assert_eq!(list[2].as_int().unwrap(), 3);
}

#[test]
fn safe_stdlib_prepend_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[2 3] | prepend 1";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 3);
	assert_eq!(list[0].as_int().unwrap(), 1);
}

#[test]
fn safe_stdlib_flatten_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[1 2] [3] 4] | flatten";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 4);
	assert_eq!(list[0].as_int().unwrap(), 1);
	assert_eq!(list[3].as_int().unwrap(), 4);
}

#[test]
fn safe_stdlib_compact_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 null 2 null] | compact";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list.len(), 2);
	assert_eq!(list[0].as_int().unwrap(), 1);
	assert_eq!(list[1].as_int().unwrap(), 2);
}

#[test]
fn safe_stdlib_flatten_rejects_over_max_items() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	// Each inner list has 2 items, 5001 of them = 10002 items after flatten
	let inner = "[1 2] ".repeat(5001);
	let source = format!("[{inner}] | flatten");
	let parsed = parse_and_validate(&mut engine_state, "<test>", &source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(
		err_str.contains("sandbox limit exceeded") || err_str.contains("exceeds"),
		"should hit flatten limit: {err_str}"
	);
}

#[test]
fn safe_stdlib_sort_ints_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[3 1 2] | sort";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_int().unwrap(), 1);
	assert_eq!(list[1].as_int().unwrap(), 2);
	assert_eq!(list[2].as_int().unwrap(), 3);
}

#[test]
fn safe_stdlib_sort_strings_reverse_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "['a' 'c' 'b'] | sort --reverse";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_str().unwrap(), "c");
	assert_eq!(list[1].as_str().unwrap(), "b");
	assert_eq!(list[2].as_str().unwrap(), "a");
}

#[test]
fn safe_stdlib_sort_by_int_column_works() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[name age]; [Alice 30] [Bob 25]] | sort-by age";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_record().unwrap().get("name").unwrap().as_str().unwrap(), "Bob");
	assert_eq!(list[1].as_record().unwrap().get("name").unwrap().as_str().unwrap(), "Alice");
}

#[test]
fn safe_stdlib_sort_rejects_mixed_types() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[1 'a'] | sort";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(
		result.is_err() && (err_str.contains("mixed") || err_str.contains("Mixed")),
		"should reject mixed types: {err_str}"
	);
}

#[test]
fn safe_stdlib_sort_nulls_last_default() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[null 2 1] | sort";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_int().unwrap(), 1);
	assert_eq!(list[1].as_int().unwrap(), 2);
	assert!(list[2].is_nothing());
}

#[test]
fn safe_stdlib_sort_nulls_first() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[null 2 1] | sort --nulls-first";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert!(list[0].is_nothing());
	assert_eq!(list[1].as_int().unwrap(), 1);
	assert_eq!(list[2].as_int().unwrap(), 2);
}

#[test]
fn safe_stdlib_sort_by_nulls_first() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[a]; [null] [2] [1]] | sort-by a --nulls-first";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	let first_a = list[0].as_record().unwrap().get("a").unwrap();
	assert!(first_a.is_nothing());
}

#[test]
fn safe_stdlib_sort_reverse_nulls_last() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[null 1 2] | sort --reverse";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	// --reverse reverses concrete ordering, null stays last (default nulls-last)
	assert_eq!(list[0].as_int().unwrap(), 2);
	assert_eq!(list[1].as_int().unwrap(), 1);
	assert!(list[2].is_nothing());
}

#[test]
fn safe_stdlib_sort_reverse_nulls_first() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[null 1 2] | sort --reverse --nulls-first";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	// null stays first, concrete reversed
	assert!(list[0].is_nothing());
	assert_eq!(list[1].as_int().unwrap(), 2);
	assert_eq!(list[2].as_int().unwrap(), 1);
}

#[test]
fn safe_stdlib_sort_by_reverse_nulls_last() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[a]; [null] [1] [2]] | sort-by a --reverse";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert_eq!(list[0].as_record().unwrap().get("a").unwrap().as_int().unwrap(), 2);
	assert_eq!(list[1].as_record().unwrap().get("a").unwrap().as_int().unwrap(), 1);
	assert!(list[2].as_record().unwrap().get("a").unwrap().is_nothing());
}

#[test]
fn safe_stdlib_sort_by_reverse_nulls_first() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "[[a]; [null] [1] [2]] | sort-by a --reverse --nulls-first";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	let list = value.as_list().expect("should be list");
	assert!(list[0].as_record().unwrap().get("a").unwrap().is_nothing());
	assert_eq!(list[1].as_record().unwrap().get("a").unwrap().as_int().unwrap(), 2);
	assert_eq!(list[2].as_record().unwrap().get("a").unwrap().as_int().unwrap(), 1);
}

#[test]
fn xeno_assert_passes_through_when_true() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | xeno assert true 'ok'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_int().unwrap(), 42);
}

#[test]
fn xeno_assert_fails_when_false() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = "42 | xeno assert false 'nope'";
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let result = evaluate_block(&engine_state, parsed.block.as_ref());
	let err_str = format!("{:?}", result);
	assert!(result.is_err() && err_str.contains("xeno assert failed"), "should fail: {err_str}");
}

#[test]
fn module_only_rejects_shadowing_xeno_assert() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate_with_policy(
		&mut engine_state,
		"<test>",
		r#"export def "xeno assert" [] { null }"#,
		None,
		ParsePolicy::ModuleOnly,
	)
	.expect_err("shadowing 'xeno assert' should be rejected");
	assert!(err.contains("reserved") && err.contains("xeno assert"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Sandbox negative regression: AST-level bans (18A)
// ---------------------------------------------------------------------------

#[test]
fn sandbox_rejects_external_command() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate(&mut engine_state, "<test>", "^echo hi", None).expect_err("external commands should be rejected");
	// May be caught at compile time or AST scan level
	assert!(err.to_lowercase().contains("external") || err.contains("not supported"), "got: {err}");
}

#[test]
fn sandbox_rejects_pipeline_redirection() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate(&mut engine_state, "<test>", "echo hi out> /tmp/out.txt", None).expect_err("pipeline redirection should be rejected");
	assert!(err.contains("pipeline redirection is disabled"), "got: {err}");
}

#[test]
fn sandbox_rejects_range_expression() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let err = parse_and_validate(&mut engine_state, "<test>", "echo 1..10", None).expect_err("range expressions should be rejected");
	assert!(err.contains("range") && err.contains("disabled"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Sandbox negative regression: forbidden commands not registered (18B)
// ---------------------------------------------------------------------------

#[test]
fn safe_stdlib_sort_rejects_over_max_items() {
	let items: Vec<String> = (0..10001).map(|i| i.to_string()).collect();
	let list_literal = format!("[{}]", items.join(" "));
	let source = format!("{list_literal} | sort");
	let mut engine_state = create_engine_state(None).expect("engine state");
	let parsed = parse_and_validate(&mut engine_state, "<test>", &source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("sort should reject >10000 items");
	assert!(err.contains("exceeds") || err.contains("limit"), "got: {err}");
}

#[test]
fn safe_stdlib_append_rejects_over_max_items() {
	let items: Vec<String> = (0..10000).map(|i| i.to_string()).collect();
	let list_literal = format!("[{}]", items.join(" "));
	let source = format!("{list_literal} | append 1");
	let mut engine_state = create_engine_state(None).expect("engine state");
	let parsed = parse_and_validate(&mut engine_state, "<test>", &source, None).expect("should parse");
	let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("append should reject at MAX_ITEMS");
	assert!(err.contains("exceeds") || err.contains("limit"), "got: {err}");
}

#[test]
fn sandbox_rejects_forbidden_commands() {
	let forbidden_sources = [
		"for x in [1 2] { $x }",
		"while true { echo hi }",
		"loop { echo hi }",
		"overlay use foo",
		"extern foo []",
	];
	for source in &forbidden_sources {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let err = parse_and_validate(&mut engine_state, "<test>", source, None).expect_err(&format!("should reject forbidden source: {source}"));
		assert!(
			err.contains("error") || err.contains("not found") || err.contains("disabled"),
			"unexpected error for '{source}': {err}"
		);
	}
}

// ---------------------------------------------------------------------------
// Schema integration test (23B)
// ---------------------------------------------------------------------------

#[test]
fn action_command_uses_schema_constants() {
	use xeno_invocation::schema;

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

// ---------------------------------------------------------------------------
// xeno emit tests (24C)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// xeno emit-many + xeno is-invocation tests (25D)
// ---------------------------------------------------------------------------

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
fn xeno_is_invocation_true_for_valid() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"{kind: "action", name: "x"} | xeno is-invocation"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_bool().unwrap(), true);
}

#[test]
fn xeno_is_invocation_false_for_non_record() {
	let mut engine_state = create_engine_state(None).expect("engine state");
	let source = r#"42 | xeno is-invocation"#;
	let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
	let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
	assert_eq!(value.as_bool().unwrap(), false);
}

#[test]
fn module_only_rejects_shadowing_xeno_emit_many_and_is_invocation() {
	for name in ["xeno emit-many", "xeno is-invocation"] {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = format!(r#"export def "{name}" [] {{ null }}"#);
		let err = parse_and_validate_with_policy(&mut engine_state, "<test>", &source, None, ParsePolicy::ModuleOnly)
			.expect_err(&format!("shadowing '{name}' should be rejected"));
		assert!(err.contains("reserved") && err.contains(name), "got: {err}");
	}
}
