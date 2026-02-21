use super::*;
use crate::types::{Invocation, InvocationStatus};

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
		Invocation::Command(xeno_invocation::CommandInvocation {
			route: xeno_invocation::CommandRoute::Auto,
			..
		})
	));
	assert!(matches!(
		crate::nu::parse_invocation_spec("editor:stats").expect("editor command should parse"),
		Invocation::Command(xeno_invocation::CommandInvocation {
			route: xeno_invocation::CommandRoute::Editor,
			..
		})
	));
	assert!(matches!(
		crate::nu::parse_invocation_spec("nu:go").expect("nu command should parse"),
		Invocation::Nu { .. }
	));
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_dispatches_action() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def go [name] { xeno effect dispatch action $name | xeno effects normalize }",
	);

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

	let result = editor
		.run_invocation(
			Invocation::editor_command("nu-run", vec!["go".to_string(), action_name]),
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(matches!(result.status, InvocationStatus::Ok));
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_command_injects_ctx() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def go [] { if $env.XENO_CTX.kind == \"macro\" { xeno effect dispatch action move_right } else { xeno effect dispatch action does-not-exist } | xeno effects normalize }",
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let args = ["go"];
	let outcome = {
		let mut ctx = EditorCommandContext {
			editor: &mut editor,
			args: &args,
		};
		cmd_nu_run(&mut ctx).await
	}
	.expect("nu-run should succeed");

	assert!(matches!(outcome, CommandOutcome::Ok));
	assert_eq!(editor.buffer().cursor, 1, "ctx-aware macro should dispatch move_right");
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_noop_macro_returns_ok() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def noop [] { null }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor.run_invocation(Invocation::nu("noop", vec![]), InvocationPolicy::enforcing()).await;

	assert!(
		matches!(result.status, InvocationStatus::Ok),
		"null-returning macro should be Ok, got: {result:?}"
	);
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_command_injects_expanded_ctx_fields() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		r#"export def go [] {
  let c = $env.XENO_CTX
  if ($c.kind == "macro") and ($c.view.id == 1) and ($c.cursor.line == 0) and ($c.cursor.col == 0) and ($c.selection.active == false) and ($c.selection.start.line == 0) and ($c.selection.start.col == 0) and ($c.selection.end.line == 0) and ($c.selection.end.col == 0) and ($c.buffer.path == null) and ($c.buffer.file_type == null) and ($c.buffer.modified == false) and ($c.buffer.readonly == false) { xeno effect dispatch action move_right } else { xeno effect dispatch action does-not-exist } | xeno effects normalize
}"#,
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing())
		.await;

	assert!(matches!(result.status, InvocationStatus::Ok));
	assert_eq!(editor.buffer().cursor, 1, "expanded ctx fields should be available to macro scripts");
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_dispatches_editor_command() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { xeno effect dispatch editor stats | xeno effects normalize }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing())
		.await;

	assert!(matches!(result.status, InvocationStatus::Ok));
}

#[tokio::test(flavor = "current_thread")]
async fn nu_reload_rejects_external_script_and_keeps_existing_runtime() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def ok [] { xeno effect dispatch editor stats | xeno effects normalize }");

	let mut editor = Editor::new_scratch();
	let initial_runtime = crate::nu::NuRuntime::load(temp.path()).expect("initial runtime should load");
	let initial_script = initial_runtime.script_path().to_path_buf();
	editor.set_nu_runtime(Some(initial_runtime));

	write_script(temp.path(), "^echo hi");

	let err = reload_runtime_from_dir(&mut editor, temp.path().to_path_buf())
		.await
		.expect_err("external scripts should be rejected");

	assert!(matches!(err, CommandError::Failed(_)));
	let kept_runtime = editor.nu_runtime().expect("existing runtime should be kept");
	assert_eq!(kept_runtime.script_path(), initial_script);
}

#[tokio::test(flavor = "current_thread")]
async fn action_post_hook_dispatches_once_with_recursion_guard() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def on_hook [] { let evt = $env.XENO_CTX.event; if $evt.type == \"action_post\" and $evt.data.name == \"move_right\" and $evt.data.result == \"ok\" { xeno effect dispatch action move_right } else { [] } | xeno effects normalize }",
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let result = editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()).await;
	editor.drain_nu_hook_queue(usize::MAX).await;

	assert!(matches!(result.status, InvocationStatus::Ok));
	assert_eq!(editor.buffer().cursor, 2, "hook should add exactly one extra move_right invocation");
}

#[tokio::test(flavor = "current_thread")]
async fn action_post_hook_receives_expanded_ctx_fields() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		r#"export def on_hook [] {
  let c = $env.XENO_CTX
  let evt = $c.event
  if ($c.kind == "hook") and ($evt.type == "action_post") and ($evt.data.name == "move_right") and ($evt.data.result == "ok") and ($c.view.id == 1) and ($c.cursor.line == 0) and ($c.cursor.col == 1) and ($c.selection.active == false) and ($c.selection.start.col == 1) and ($c.selection.end.col == 1) and ($c.buffer.modified == false) and ($c.buffer.readonly == false) { xeno effect dispatch action move_right } else { [] } | xeno effects normalize
}"#,
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let result = editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()).await;
	editor.drain_nu_hook_queue(usize::MAX).await;

	assert!(matches!(result.status, InvocationStatus::Ok));
	assert_eq!(editor.buffer().cursor, 2, "expanded ctx fields should be available to hook scripts");
}

#[tokio::test(flavor = "current_thread")]
async fn action_post_missing_hook_is_noop() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def unrelated [] { [] }");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let result = editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()).await;

	assert!(matches!(result.status, InvocationStatus::Ok));
	assert_eq!(editor.buffer().cursor, 1, "without on_hook export only base action should run");
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_structured_action_record_executes_count() {
	assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def go [] { xeno effect dispatch action move_right --count 2 | xeno effects normalize }",
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::from_content("abcd".to_string(), None);
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing())
		.await;

	assert!(matches!(result.status, InvocationStatus::Ok));
	assert_eq!(editor.buffer().cursor, 2, "structured action record should honor count");
}

#[tokio::test(flavor = "current_thread")]
async fn nu_run_structured_list_of_records_executes() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(
		temp.path(),
		"export def go [] { [ (xeno effect dispatch editor stats), (xeno effect dispatch command help) ] | xeno effects normalize }",
	);

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing())
		.await;

	assert!(matches!(result.status, InvocationStatus::Ok));
}
