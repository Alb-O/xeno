use super::*;
use crate::types::Invocation;

fn write_script(dir: &std::path::Path, source: &str) {
	std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
}

#[test]
fn parse_invocation_variants() {
	assert!(matches!(
		crate::nu::parse_invocation_spec("action:move_right").expect("action should parse"),
		Invocation::Action { .. }
	));
	assert!(matches!(
		crate::nu::parse_invocation_spec("command:help themes").expect("command should parse"),
		Invocation::Command { .. }
	));
	assert!(matches!(
		crate::nu::parse_invocation_spec("editor:stats").expect("editor command should parse"),
		Invocation::EditorCommand { .. }
	));
}

#[test]
fn nu_run_dispatches_action() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [name] { $\"action:($name)\" }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let action_name = if xeno_registry::find_action("move_right").is_some() {
		"move_right".to_string()
	} else {
		xeno_registry::all_actions()
			.first()
			.map(|action| action.name_str().to_string())
			.expect("registry should include at least one action")
	};

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let result = rt.block_on(editor.run_invocation(
		Invocation::editor_command("nu-run", vec!["go".to_string(), action_name]),
		InvocationPolicy::enforcing(),
	));

	assert!(matches!(result, InvocationResult::Ok));
}

#[test]
fn nu_run_dispatches_editor_command() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { \"editor:stats\" }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let result = rt.block_on(editor.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing()));

	assert!(matches!(result, InvocationResult::Ok));
}

#[test]
fn nu_reload_rejects_external_script_and_keeps_existing_runtime() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def ok [] { \"editor:stats\" }");

	let mut editor = Editor::new_scratch();
	let initial_runtime = crate::nu::NuRuntime::load(temp.path()).expect("initial runtime should load");
	let initial_script = initial_runtime.script_path().to_path_buf();
	editor.set_nu_runtime(Some(initial_runtime));

	write_script(temp.path(), "^echo hi");

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let err = rt
		.block_on(reload_runtime_from_dir(&mut editor, temp.path().to_path_buf()))
		.expect_err("external scripts should be rejected");

	assert!(matches!(err, CommandError::Failed(_)));
	let kept_runtime = editor.nu_runtime().expect("existing runtime should be kept");
	assert_eq!(kept_runtime.script_path(), initial_script);
}

#[test]
fn action_post_hook_dispatches_once_with_recursion_guard() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def on_action_post [name result] { if $name == \"move_right\" and $result == \"ok\" { \"action:move_right\" } else { [] } }",
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let result = rt.block_on(editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()));

	assert!(matches!(result, InvocationResult::Ok));
	assert_eq!(editor.buffer().cursor, 2, "hook should add exactly one extra move_right invocation");
}

#[test]
fn action_post_missing_hook_is_noop() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def unrelated [] { [] }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let result = rt.block_on(editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()));

	assert!(matches!(result, InvocationResult::Ok));
	assert_eq!(editor.buffer().cursor, 1, "without on_action_post hook only base action should run");
}

#[test]
fn nu_run_structured_action_record_executes_count() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { { kind: \"action\", name: \"move_right\", count: 2 } }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let result = rt.block_on(editor.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing()));

	assert!(matches!(result, InvocationResult::Ok));
	assert_eq!(editor.buffer().cursor, 2, "structured action record should honor count");
}

#[test]
fn nu_run_structured_list_of_records_executes() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def go [] { [\n  { kind: \"editor\", name: \"stats\" },\n  { kind: \"command\", name: \"help\" }\n] }",
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("runtime should build");
	let result = rt.block_on(editor.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing()));

	assert!(matches!(result, InvocationResult::Ok));
}
