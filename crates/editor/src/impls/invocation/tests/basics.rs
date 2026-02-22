use super::*;

#[test]
fn invocation_describe() {
	assert_eq!(Invocation::action("move_left").describe(), "action:move_left");
	assert_eq!(Invocation::action_with_count("move_down", 5).describe(), "action:move_downx5");
	assert_eq!(Invocation::command("write", vec!["file.txt".into()]).describe(), "cmd:write file.txt");
	assert_eq!(Invocation::editor_command("quit", vec![]).describe(), "editor_cmd:quit");
	assert_eq!(Invocation::nu("go", vec!["fast".into()]).describe(), "nu:go fast");
}

#[test]
fn invocation_policy_defaults() {
	let policy = InvocationPolicy::default();
	assert!(!policy.enforce_readonly);

	let policy = InvocationPolicy::enforcing();
	assert!(policy.enforce_readonly);
}

#[tokio::test]
async fn action_hooks_fire_once() {
	// Test defs registered via inventory::submit!(BuiltinsReg) at DB init time.
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let mut editor = Editor::new_scratch();
	let result = editor.invoke_action("invocation_test_action", 1, false, None, None).await;
	assert!(matches!(result.status, InvocationStatus::Ok));

	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());

	assert_eq!(pre_count, 1);
	assert_eq!(post_count, 1);
}

#[tokio::test]
async fn readonly_enforcement_blocks_edit_actions() {
	// Test defs registered via inventory::submit!(BuiltinsReg) at DB init time.
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let result = editor.run_action_invocation("invocation_edit_action", 1, false, None, None, InvocationPolicy::enforcing());

	assert!(matches!(result.status, InvocationStatus::ReadonlyDenied));
}

#[tokio::test]
async fn readonly_disabled_allows_edit_actions() {
	// Test defs registered via inventory::submit!(BuiltinsReg) at DB init time.
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let result = editor.run_action_invocation("invocation_edit_action", 1, false, None, None, InvocationPolicy::log_only());

	assert!(matches!(result.status, InvocationStatus::Ok));
}

#[tokio::test]
async fn command_error_propagates() {
	// Test defs registered via inventory::submit!(BuiltinsReg) at DB init time.
	let mut editor = Editor::new_scratch();
	let result = editor
		.run_invocation(
			Invocation::command("invocation_test_command_fail".to_string(), vec![]),
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(matches!(result.status, InvocationStatus::CommandError));
	assert!(
		result.detail_text().is_some_and(|msg| msg.contains("boom")),
		"expected command error detail to include boom, got: {result:?}"
	);
}

#[tokio::test]
async fn action_count_usize_max_clamped_at_engine_boundary() {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(0));

	let mut editor = Editor::new_scratch();
	let result = editor
		.run_invocation(
			Invocation::Action {
				name: "invocation_test_action".to_string(),
				count: usize::MAX,
				extend: false,
				register: None,
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(
		matches!(result.status, InvocationStatus::Ok),
		"action with clamped usize::MAX count should succeed, got: {result:?}"
	);
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|c| c.get()),
		1,
		"action handler should be invoked exactly once with clamped count"
	);
}
