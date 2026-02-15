use super::*;

fn write_script(dir: &Path, source: &str) {
	std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
}

#[test]
fn load_rejects_external_calls() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "^echo hi");
	let err = NuRuntime::load(temp.path()).expect_err("external calls should be rejected");
	let err_lower = err.to_lowercase();
	assert!(err_lower.contains("external") || err_lower.contains("parse error"), "{err}");
}

#[test]
fn run_invocations_supports_record_and_list() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def one [] { editor stats }\nexport def many [] { [(editor stats), (command help)] }",
	);

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

	let one = runtime.run_invocations("one", &[]).expect("record return should decode");
	assert!(matches!(one.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));

	let many = runtime.run_invocations("many", &[]).expect("list return should decode");
	assert_eq!(many.len(), 2);
}

#[test]
fn run_invocations_supports_alias_entrypoint() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export alias go = editor stats");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let invocations = runtime.run_invocations("go", &[]).expect("alias entrypoint should run");
	assert!(matches!(invocations.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
}

#[test]
fn run_invocations_supports_structured_records() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def action_rec [] { { kind: \"action\", name: \"move_right\", count: 2, extend: true, register: \"a\" } }\n\
export def action_char [] { { kind: \"action\", name: \"find_char\", char: \"x\" } }\n\
export def mixed [] { [ { kind: \"editor\", name: \"stats\" }, { kind: \"command\", name: \"help\", args: [\"themes\"] } ] }\n\
export def nested_nu [] { { kind: \"nu\", name: \"go\", args: [\"a\", \"b\"] } }",
	);

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

	let action = runtime.run_invocations("action_rec", &[]).expect("structured action should decode");
	assert!(matches!(
		action.as_slice(),
		[Invocation::Action {
			name,
			count: 2,
			extend: true,
			register: Some('a')
		}] if name == "move_right"
	));

	let action_char = runtime.run_invocations("action_char", &[]).expect("structured action-with-char should decode");
	assert!(matches!(
		action_char.as_slice(),
		[Invocation::ActionWithChar {
			name,
			char_arg: 'x',
			..
		}] if name == "find_char"
	));

	let mixed = runtime.run_invocations("mixed", &[]).expect("structured list should decode");
	assert!(matches!(mixed.first(), Some(Invocation::EditorCommand { name, .. }) if name == "stats"));
	assert!(matches!(mixed.get(1), Some(Invocation::Command { name, .. }) if name == "help"));

	let nested_nu = runtime.run_invocations("nested_nu", &[]).expect("structured nu invocation should decode");
	assert!(matches!(nested_nu.as_slice(), [Invocation::Nu { name, args }] if name == "go" && args == &["a".to_string(), "b".to_string()]));
}

#[test]
fn decode_limits_cap_invocation_count() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def many [] { [(editor stats), (editor stats)] }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let err = runtime
		.run_invocations_with_limits(
			"many",
			&[],
			DecodeLimits {
				max_invocations: 1,
				..DecodeLimits::macro_defaults()
			},
		)
		.expect_err("decode limits should reject too many invocations");

	assert!(err.contains("invocation count"), "{err}");
}

#[test]
fn run_allows_use_within_config_root() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(temp.path().join("mod.nu"), "export def mk [] { editor stats }").expect("module should be writable");
	write_script(temp.path(), "use mod.nu *\nexport def go [] { mk }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let invocations = runtime.run_invocations("go", &[]).expect("run should succeed");
	assert!(matches!(invocations.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
}

#[test]
fn try_run_returns_none_for_missing_function() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def known [] { editor stats }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let missing = runtime.try_run_invocations("missing", &[]).expect("missing function should be non-fatal");
	assert!(missing.is_none());
}

#[test]
fn find_script_decl_rejects_builtins() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { editor stats }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	assert!(runtime.find_script_decl("go").is_some());
	assert!(runtime.find_script_decl("if").is_none());
	assert!(runtime.find_script_decl("nonexistent").is_none());
}

#[test]
fn run_rejects_builtin_decls() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { editor stats }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

	let err = runtime.run("if", &[]).expect_err("builtin 'if' should be rejected");
	assert!(err.contains("not defined"), "expected 'not defined' error, got: {err}");

	let err = runtime.run("for", &[]).expect_err("builtin 'for' should be rejected");
	assert!(err.contains("not defined"), "expected 'not defined' error, got: {err}");

	let _ = runtime.run("go", &[]).expect("script function should succeed");
}

#[test]
fn nothing_return_decodes_to_empty_invocations() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def noop [] { null }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let result = runtime.run_invocations("noop", &[]).expect("nothing return should decode");
	assert!(result.is_empty());
}

#[test]
fn load_rejects_top_level_statement() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "42");
	let err = NuRuntime::load(temp.path()).expect_err("top-level expression should be rejected");
	assert!(err.contains("top-level") || err.contains("module-only"), "{err}");
}

#[test]
fn load_rejects_export_extern_top_level() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export extern git []");
	let err = NuRuntime::load(temp.path()).expect_err("export extern should be rejected");
	assert!(
		err.contains("not allowed") || err.contains("extern") || err.contains("parse error") || err.contains("Unknown"),
		"{err}"
	);
}

#[test]
fn load_allows_const_used_by_macro() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "const CMD = \"stats\"\nexport def go [] { editor $CMD }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let invocations = runtime.run_invocations("go", &[]).expect("run should succeed");
	assert!(matches!(invocations.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
}
