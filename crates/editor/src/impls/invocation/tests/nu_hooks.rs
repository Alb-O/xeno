use super::*;

#[tokio::test]
async fn nu_hook_ctx_is_injected() {
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] { if $env.XENO_CTX.kind == \"hook\" { (xeno effect dispatch action invocation_test_action) } else { (xeno effect dispatch action does-not-exist) } | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(Invocation::action("invocation_test_action"), InvocationPolicy::enforcing())
		.await;
	editor.drain_nu_hook_queue(usize::MAX).await;
	assert!(matches!(result.status, InvocationStatus::Ok));

	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());
	assert_eq!(pre_count, 2);
	assert_eq!(post_count, 2);
}

#[tokio::test]
async fn nu_hook_capability_denial_is_non_fatal() {
	INVOCATION_TEST_ACTION_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] { xeno effect dispatch action invocation_test_action | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));
	editor.state.config.config.nu = Some(xeno_registry::config::NuConfig {
		budget_macro: None,
		budget_hook: None,
		permissions_macro: None,
		permissions_hook: Some(HashSet::new()),
	});

	let result = editor
		.run_invocation(Invocation::action("invocation_test_action"), InvocationPolicy::enforcing())
		.await;
	editor.drain_nu_hook_queue(usize::MAX).await;

	assert!(matches!(result.status, InvocationStatus::Ok), "base invocation should remain successful");
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|count| count.get()),
		1,
		"hook-side denial should suppress extra dispatch without failing base invocation"
	);
}

#[tokio::test]
async fn nu_hook_stop_propagation_clears_queued_work() {
	INVOCATION_TEST_ACTION_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] {\n\
  let evt = $env.XENO_CTX.event\n\
  if $evt.type == \"action_post\" {\n\
    return (xeno effect stop | xeno effects normalize)\n\
  }\n\
  if $evt.type == \"command_post\" {\n\
    return (xeno effect dispatch action invocation_test_action | xeno effects normalize)\n\
  }\n\
}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let _ = editor
		.run_invocation(Invocation::action("invocation_test_action"), InvocationPolicy::enforcing())
		.await;
	let _ = editor
		.run_invocation(Invocation::command("invocation_test_command_fail", vec![]), InvocationPolicy::enforcing())
		.await;

	editor.drain_nu_hook_queue(usize::MAX).await;

	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|count| count.get()),
		1,
		"stop-propagation should prevent later queued hook dispatches"
	);
}

#[tokio::test]
async fn nu_runtime_reload_swaps_executor_and_disables_old_runtime_hooks() {
	INVOCATION_TEST_ACTION_COUNT.with(|count| count.set(0));
	INVOCATION_TEST_ACTION_ALT_COUNT.with(|count| count.set(0));

	let temp_a = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp_a.path().join("xeno.nu"),
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"action_post\" and $evt.data.name == \"invocation_edit_action\" and $evt.data.result == \"ok\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let temp_b = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp_b.path().join("xeno.nu"),
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"action_post\" and $evt.data.name == \"invocation_edit_action\" and $evt.data.result == \"ok\" {\n\
				(xeno effect dispatch action invocation_test_action_alt)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime_a = crate::nu::NuRuntime::load(temp_a.path()).expect("runtime A should load");
	let runtime_b = crate::nu::NuRuntime::load(temp_b.path()).expect("runtime B should load");

	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime_a));

	let result_a = editor
		.run_invocation(Invocation::action("invocation_edit_action"), InvocationPolicy::enforcing())
		.await;
	editor.drain_nu_hook_queue(usize::MAX).await;
	assert!(matches!(result_a.status, InvocationStatus::Ok));
	assert_eq!(INVOCATION_TEST_ACTION_COUNT.with(|count| count.get()), 1, "runtime A hook should run once");
	assert_eq!(
		INVOCATION_TEST_ACTION_ALT_COUNT.with(|count| count.get()),
		0,
		"runtime B hook should not run yet"
	);

	let old_shared = editor
		.state
		.integration
		.nu
		.executor()
		.expect("executor should exist after first Nu hook execution")
		.shutdown_acks_for_tests();
	assert_eq!(
		old_shared.shutdown_acks.load(std::sync::atomic::Ordering::SeqCst),
		0,
		"old executor should not be acked before swap"
	);

	editor.set_nu_runtime(Some(runtime_b));
	assert_eq!(
		old_shared.shutdown_acks.load(std::sync::atomic::Ordering::SeqCst),
		1,
		"old executor should ack shutdown during runtime swap"
	);

	let result_b = editor
		.run_invocation(Invocation::action("invocation_edit_action"), InvocationPolicy::enforcing())
		.await;
	editor.drain_nu_hook_queue(usize::MAX).await;
	assert!(matches!(result_b.status, InvocationStatus::Ok));
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|count| count.get()),
		1,
		"runtime A action must not run again after reload"
	);
	assert_eq!(
		INVOCATION_TEST_ACTION_ALT_COUNT.with(|count| count.get()),
		1,
		"runtime B hook should run after reload"
	);
}

#[tokio::test]
async fn command_post_hook_runs_and_receives_args() {
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		// Hook fires invocation_test_action only when it receives the expected event data.
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"command_post\" and $evt.data.name == \"invocation_test_command_fail\" and $evt.data.result == \"error\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	// Run the failing command — its post-hook should fire the test action.
	let result = editor
		.run_invocation(Invocation::command("invocation_test_command_fail", vec![]), InvocationPolicy::enforcing())
		.await;
	editor.drain_nu_hook_queue(usize::MAX).await;

	// The command itself fails. The hook enqueues and drain runs it, firing
	// the test action. The original command error result is preserved.
	assert!(matches!(result.status, InvocationStatus::CommandError));
	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());
	assert_eq!(pre_count, 1, "hook-produced action should fire pre hook");
	assert_eq!(post_count, 1, "hook-produced action should fire post hook");
}

#[tokio::test]
async fn editor_command_post_hook_runs() {
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"editor_command_post\" and $evt.data.name == \"stats\" and $evt.data.result == \"ok\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(Invocation::editor_command("stats", vec![]), InvocationPolicy::enforcing())
		.await;
	editor.drain_nu_hook_queue(usize::MAX).await;

	// Hook should have fired the test action.
	assert!(matches!(result.status, InvocationStatus::Ok));
	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());
	assert_eq!(pre_count, 1, "hook-produced action should fire pre hook");
	assert_eq!(post_count, 1, "hook-produced action should fire post hook");
}

#[tokio::test]
async fn mode_change_hook_runs_on_transition() {
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"mode_change\" and $evt.data.from == \"Normal\" and $evt.data.to == \"Insert\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	assert!(editor.state.integration.nu.hook_id().on_hook.is_some(), "decl ID should be cached");

	editor.enqueue_mode_change_hook(&xeno_primitives::Mode::Normal, &xeno_primitives::Mode::Insert);
	let quit = editor.drain_nu_hook_queue(usize::MAX).await;

	assert!(!quit);
	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());
	assert_eq!(pre_count, 1, "hook-produced action should fire");
	assert_eq!(post_count, 1, "hook-produced action should fire");
}

#[tokio::test]
async fn mode_change_hook_does_not_run_for_non_matching_transition() {
	ACTION_PRE_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"mode_change\" and $evt.data.from == \"Normal\" and $evt.data.to == \"Insert\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	editor.enqueue_mode_change_hook(&xeno_primitives::Mode::Normal, &xeno_primitives::Mode::Normal);
	let quit = editor.drain_nu_hook_queue(usize::MAX).await;

	assert!(!quit);
	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	assert_eq!(pre_count, 0, "no action should fire for non-matching transition");
}

#[tokio::test]
async fn mode_change_hook_fires_on_insert_key() {
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] {\n\
			let evt = $env.XENO_CTX.event\n\
			if $evt.type == \"mode_change\" and $evt.data.from == \"Normal\" and $evt.data.to == \"Insert\" and $env.XENO_CTX.mode == \"Insert\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));
	assert_eq!(editor.mode(), Mode::Normal);

	// Press 'i' — the default keybinding for entering Insert mode.
	let quit = editor.handle_key(Key::new(KeyCode::Char('i'))).await;
	assert!(!quit);
	assert_eq!(editor.mode(), Mode::Insert);

	// Hooks are now deferred — drain to execute them.
	editor.drain_nu_hook_queue(usize::MAX).await;

	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());
	assert_eq!(pre_count, 1, "hook-produced action should fire via key handling");
	assert_eq!(post_count, 1, "hook-produced action should fire via key handling");
}

#[tokio::test]
async fn mode_change_hook_does_not_fire_when_mode_unchanged() {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		// Unconditional — fires for any mode change, so if guard fails we catch it.
		"export def on_hook [] {\n\
			if $env.XENO_CTX.event.type == \"mode_change\" {\n\
				(xeno effect dispatch action invocation_test_action)\n\
			} | xeno effects normalize\n\
		}",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));
	assert_eq!(editor.mode(), Mode::Normal);

	// Press 'l' — a motion key that doesn't change mode.
	let quit = editor.handle_key(Key::new(KeyCode::Char('l'))).await;
	assert!(!quit);
	assert_eq!(editor.mode(), Mode::Normal);

	let count = INVOCATION_TEST_ACTION_COUNT.with(|c| c.get());
	assert_eq!(count, 0, "no mode change should mean no hook-produced action");
}

#[tokio::test]
async fn buffer_open_hook_fires_on_disk_open() {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	let file_path = temp.path().join("test.txt");
	std::fs::write(&file_path, "hello world").expect("write test file");

	let nu_dir = tempfile::tempdir().expect("nu temp dir");
	std::fs::write(
		nu_dir.path().join("xeno.nu"),
		r#"export def "str ends-with" [suffix: string] { $in ends-with $suffix }
export def on_hook [] {
  let evt = $env.XENO_CTX.event
  if $evt.type == "buffer_open" and ($evt.data.path | str ends-with "test.txt") and $evt.data.kind == "disk" {
    (xeno effect dispatch action invocation_test_action)
  } | xeno effects normalize
}"#,
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(nu_dir.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	assert!(editor.state.integration.nu.hook_id().on_hook.is_some(), "decl ID should be cached");

	let location = crate::impls::navigation::Location {
		path: file_path,
		line: 0,
		column: 0,
	};
	editor.goto_location(&location).await.expect("goto should succeed");
	editor.drain_nu_hook_queue(usize::MAX).await;

	let count = INVOCATION_TEST_ACTION_COUNT.with(|c| c.get());
	assert_eq!(count, 1, "on_hook (buffer_open) should fire exactly once for disk open");
}

#[tokio::test]
async fn buffer_open_hook_fires_for_existing_switch() {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	let file_a = temp.path().join("a.txt");
	let file_b = temp.path().join("b.txt");
	std::fs::write(&file_a, "aaa").expect("write a");
	std::fs::write(&file_b, "bbb").expect("write b");

	let nu_dir = tempfile::tempdir().expect("nu temp dir");
	std::fs::write(
		nu_dir.path().join("xeno.nu"),
		r#"export def on_hook [] {
  let evt = $env.XENO_CTX.event
  if $evt.type == "buffer_open" and $evt.data.kind == "existing" {
    (xeno effect dispatch action invocation_test_action)
  } | xeno effects normalize
}"#,
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(nu_dir.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.set_nu_runtime(Some(runtime));

	// Open file A in a split so it stays alive when we navigate away.
	let _view_a = editor.open_file(file_a.clone()).await.expect("open a");
	// Open file B in a second split.
	let _view_b = editor.open_file(file_b.clone()).await.expect("open b");

	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|c| c.get()),
		0,
		"disk opens via open_file should not fire existing hook"
	);

	// Navigate focused view to A — A is already open in another view → "existing".
	let loc_a = crate::impls::navigation::Location {
		path: file_a.clone(),
		line: 0,
		column: 0,
	};
	editor.goto_location(&loc_a).await.expect("switch to a");
	editor.drain_nu_hook_queue(usize::MAX).await;
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|c| c.get()),
		1,
		"existing switch should fire hook exactly once"
	);
}

#[tokio::test]
async fn nu_stats_reflect_hook_pipeline_state() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def on_hook [] { xeno effect dispatch action invocation_test_action | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let script_path = runtime.script_path().to_string_lossy().to_string();
	let mut editor = Editor::new_scratch();

	// Before loading: stats should show no runtime.
	let stats = editor.stats_snapshot();
	assert!(!stats.nu.runtime_loaded);
	assert!(!stats.nu.executor_alive);

	editor.set_nu_runtime(Some(runtime));

	// After loading: runtime present, executor created.
	let stats = editor.stats_snapshot();
	assert!(stats.nu.runtime_loaded);
	assert!(stats.nu.executor_alive);
	assert_eq!(stats.nu.script_path, Some(script_path));
	assert_eq!(stats.nu.hook_queue_len, 0);

	// Enqueue a hook and check queue length.
	editor.enqueue_action_post_hook("invocation_test_action".to_string(), &InvocationOutcome::ok(InvocationTarget::Action));
	let stats = editor.stats_snapshot();
	assert_eq!(stats.nu.hook_queue_len, 1, "hook should be enqueued");
	assert!(stats.nu.hook_in_flight.is_none());

	// Drain to clear.
	editor.drain_nu_hook_queue(usize::MAX).await;
	let stats = editor.stats_snapshot();
	assert_eq!(stats.nu.hook_queue_len, 0);
	assert_eq!(stats.nu.hook_dropped_total, 0);
}
