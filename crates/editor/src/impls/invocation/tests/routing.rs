use super::*;

#[test]
fn hook_event_builders_produce_correct_shape() {
	use crate::nu::ctx::NuCtxEvent;

	let result = InvocationOutcome::ok(InvocationTarget::Action);
	let event = super::command_post_event("write".to_string(), &result, vec!["file.txt".to_string()]);
	assert!(matches!(
		event,
		NuCtxEvent::CommandPost { name, result, args }
			if name == "write" && result == "ok" && args == vec!["file.txt".to_string()]
	));

	let action_event = super::action_post_event(
		"move_right".to_string(),
		&InvocationOutcome::command_error(InvocationTarget::Action, "boom".to_string()),
	);
	assert!(matches!(
		action_event,
		NuCtxEvent::ActionPost { name, result } if name == "move_right" && result == "error"
	));
}

#[tokio::test]
async fn all_entry_points_route_through_run_invocation() {
	use super::dispatch::run_invocation_call_count;

	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] { xeno effect dispatch action invocation_test_action | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	// Entry point 1: direct run_invocation (action).
	let before = run_invocation_call_count();
	let r = editor
		.run_invocation(Invocation::action("invocation_test_action"), InvocationPolicy::enforcing())
		.await;
	assert!(matches!(r.status, InvocationStatus::Ok));
	assert!(run_invocation_call_count() > before, "direct run_invocation should increment counter");

	// Entry point 2: key dispatch (press 'l' — a motion, routes through run_invocation).
	let before = run_invocation_call_count();
	let quit = editor.handle_key(Key::new(KeyCode::Char('l'))).await;
	assert!(!quit);
	assert!(run_invocation_call_count() > before, "key dispatch should route through run_invocation");

	// Entry point 3: hook-produced dispatch (drain queued hooks).
	let before = run_invocation_call_count();
	editor.drain_nu_hook_queue(usize::MAX).await;
	assert!(
		run_invocation_call_count() > before,
		"hook drain should route dispatches through run_invocation"
	);

	// Entry point 4: runtime work queue (enqueue + drain via run_invocation).
	let before = run_invocation_call_count();
	editor.enqueue_runtime_command_invocation(
		"invocation_test_command_fail".to_string(),
		vec![],
		crate::runtime::work_queue::RuntimeWorkSource::NuHookDispatch,
	);
	editor.drain_runtime_work_report(usize::MAX).await;
	assert!(run_invocation_call_count() > before, "runtime work drain should route through run_invocation");
}
