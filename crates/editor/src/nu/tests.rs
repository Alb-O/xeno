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
fn run_effects_supports_record_and_list() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def one [] { xeno effect dispatch editor stats }\nexport def many [] { [(xeno effect dispatch editor stats), (xeno effect dispatch command help)] }",
	);

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

	let one = runtime
		.run_macro_effects_with_budget_and_env("one", &[], DecodeBudget::macro_defaults(), &[])
		.expect("record return should decode");
	assert_eq!(one.effects.len(), 1);
	assert!(matches!(
		one.effects.as_slice(),
		[NuEffect::Dispatch(Invocation::Command(xeno_invocation::CommandInvocation {
			name,
			route: xeno_invocation::CommandRoute::Editor,
			..
		}))] if name == "stats"
	));

	let many = runtime
		.run_macro_effects_with_budget_and_env("many", &[], DecodeBudget::macro_defaults(), &[])
		.expect("list return should decode");
	assert_eq!(many.effects.len(), 2);
}

#[test]
fn run_effects_supports_structured_records() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def action_rec [] { xeno effect dispatch action move_right --count 2 --extend --register a }\n\
export def action_char [] { xeno effect dispatch action find_char --char x }\n\
export def mixed [] { [ (xeno effect dispatch editor stats), (xeno effect dispatch command help themes) ] }\n\
export def nested_nu [] { xeno call go a b }",
	);

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

	let action = runtime
		.run_macro_effects_with_budget_and_env("action_rec", &[], DecodeBudget::macro_defaults(), &[])
		.expect("structured action should decode")
		.into_dispatches();
	assert!(matches!(
		action.as_slice(),
		[Invocation::Action {
			name,
			count: 2,
			extend: true,
			register: Some('a')
		}] if name == "move_right"
	));

	let action_char = runtime
		.run_macro_effects_with_budget_and_env("action_char", &[], DecodeBudget::macro_defaults(), &[])
		.expect("structured action-with-char should decode")
		.into_dispatches();
	assert!(matches!(
		action_char.as_slice(),
		[Invocation::ActionWithChar {
			name,
			char_arg: 'x',
			..
		}] if name == "find_char"
	));

	let mixed = runtime
		.run_macro_effects_with_budget_and_env("mixed", &[], DecodeBudget::macro_defaults(), &[])
		.expect("structured list should decode")
		.into_dispatches();
	assert!(matches!(
		mixed.first(),
		Some(Invocation::Command(xeno_invocation::CommandInvocation {
			name,
			route: xeno_invocation::CommandRoute::Editor,
			..
		})) if name == "stats"
	));
	assert!(matches!(
		mixed.get(1),
		Some(Invocation::Command(xeno_invocation::CommandInvocation { name, .. })) if name == "help"
	));

	let nested_nu = runtime
		.run_macro_effects_with_budget_and_env("nested_nu", &[], DecodeBudget::macro_defaults(), &[])
		.expect("structured nu invocation should decode")
		.into_dispatches();
	assert!(matches!(nested_nu.as_slice(), [Invocation::Nu { name, args }] if name == "go" && args == &["a".to_string(), "b".to_string()]));
}

#[test]
fn decode_limits_cap_effect_count() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def many [] { [(xeno effect dispatch editor stats), (xeno effect dispatch editor stats)] }",
	);

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let err = runtime
		.run_macro_effects_with_budget_and_env(
			"many",
			&[],
			DecodeBudget {
				max_effects: 1,
				..DecodeBudget::macro_defaults()
			},
			&[],
		)
		.expect_err("decode limits should reject too many effects");

	assert!(err.contains("effect count"), "{err}");
}

#[test]
fn run_allows_use_within_config_root() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(temp.path().join("mod.nu"), "export def mk [] { xeno effect dispatch editor stats }").expect("module should be writable");
	write_script(temp.path(), "use mod.nu *\nexport def go [] { mk }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let dispatches = runtime
		.run_macro_effects_with_budget_and_env("go", &[], DecodeBudget::macro_defaults(), &[])
		.expect("run should succeed")
		.into_dispatches();
	assert!(matches!(
		dispatches.as_slice(),
		[Invocation::Command(xeno_invocation::CommandInvocation {
			name,
			route: xeno_invocation::CommandRoute::Editor,
			..
		})] if name == "stats"
	));
}

#[test]
fn find_export_rejects_builtins() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { xeno effect dispatch editor stats }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	assert!(runtime.find_export("go").is_some());
	assert!(runtime.find_export("if").is_none());
	assert!(runtime.find_export("nonexistent").is_none());
}

#[test]
fn run_rejects_builtin_decls() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { xeno effect dispatch editor stats }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

	let err = runtime.run("if", &[]).expect_err("builtin 'if' should be rejected");
	assert!(err.contains("not defined"), "expected 'not defined' error, got: {err}");

	let err = runtime.run("for", &[]).expect_err("builtin 'for' should be rejected");
	assert!(err.contains("not defined"), "expected 'not defined' error, got: {err}");

	let _ = runtime.run("go", &[]).expect("script function should succeed");
}

#[test]
fn nothing_return_decodes_to_empty_effects() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def noop [] { null }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let result = runtime
		.run_macro_effects_with_budget_and_env("noop", &[], DecodeBudget::macro_defaults(), &[])
		.expect("nothing return should decode");
	assert!(result.effects.is_empty());
}

#[test]
fn load_rejects_top_level_statement() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "42");
	let err = NuRuntime::load(temp.path()).expect_err("top-level expression should be rejected");
	assert!(
		err.contains("top-level") || err.contains("module-only") || err.contains("keyword") || err.contains("parse error"),
		"{err}"
	);
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
	write_script(temp.path(), "const CMD = \"stats\"\nexport def go [] { xeno effect dispatch editor $CMD }");

	let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
	let dispatches = runtime
		.run_macro_effects_with_budget_and_env("go", &[], DecodeBudget::macro_defaults(), &[])
		.expect("run should succeed")
		.into_dispatches();
	assert!(matches!(
		dispatches.as_slice(),
		[Invocation::Command(xeno_invocation::CommandInvocation {
			name,
			route: xeno_invocation::CommandRoute::Editor,
			..
		})] if name == "stats"
	));
}
