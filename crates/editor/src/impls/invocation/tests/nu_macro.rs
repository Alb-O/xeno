use super::*;

#[tokio::test]
async fn nu_macro_recursion_depth_guard_trips() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(temp.path().join("xeno.nu"), "export def recur [] { xeno call recur | xeno effects normalize }").expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "recur".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(matches!(result.status, InvocationStatus::CommandError));
	let msg = result.detail_text().unwrap_or_default();
	assert!(msg.to_ascii_lowercase().contains("recursion depth"), "{msg}");
}

#[tokio::test]
async fn engine_error_stops_processing_remaining_frames() {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	// Macro returns 3 dispatch effects:
	// 1. valid action (invocation_test_action) — should execute
	// 2. unknown action (does_not_exist) — should error
	// 3. valid action (invocation_test_action) — must NOT execute
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def go [] { [(xeno effect dispatch action invocation_test_action), (xeno effect dispatch action does_not_exist), (xeno effect dispatch action invocation_test_action)] | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "go".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	// Engine should stop at the error (frame 2) and not process frame 3.
	assert!(
		matches!(result.status, InvocationStatus::NotFound),
		"expected NotFound for unknown action, got: {result:?}"
	);
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|c| c.get()),
		1,
		"only the first frame's action should have executed; third frame must be skipped"
	);
}

#[tokio::test]
async fn nu_macro_mutual_recursion_depth_guard_trips() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def alpha [] { xeno call beta | xeno effects normalize }\nexport def beta [] { xeno call alpha | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "alpha".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(matches!(result.status, InvocationStatus::CommandError));
	let msg = result.detail_text().unwrap_or_default();
	assert!(
		msg.to_ascii_lowercase().contains("recursion depth"),
		"mutual recursion should trip depth guard: {msg}"
	);
}

#[tokio::test]
async fn nu_macro_ctx_is_injected() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def go [] { let ctx = (xeno ctx); if $ctx.kind == \"macro\" { (xeno effect dispatch action invocation_test_action) } else { (xeno effect dispatch action does-not-exist) } | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "go".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(matches!(result.status, InvocationStatus::Ok));
}

#[tokio::test]
async fn nu_macro_stop_effect_is_rejected() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(temp.path().join("xeno.nu"), "export def go [] { xeno effect stop | xeno effects normalize }").expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "go".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(
		matches!(result.status, InvocationStatus::CommandError)
			&& result
				.detail_text()
				.is_some_and(|msg| msg.contains("only allowed in hook") || msg.contains("hook-only stop effect")),
		"expected macro stop rejection, got: {result:?}"
	);
}

#[tokio::test]
async fn nu_macro_capability_denial_is_command_error() {
	INVOCATION_TEST_ACTION_COUNT.with(|count| count.set(0));

	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		"export def go [] { xeno effect dispatch action invocation_test_action | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));
	editor.state.config.config.nu = Some(xeno_registry::config::NuConfig {
		budget_macro: None,
		budget_hook: None,
		permissions_macro: Some(HashSet::new()),
		permissions_hook: None,
	});

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "go".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(
		matches!(result.status, InvocationStatus::CommandError) && result.detail_text().is_some_and(|msg| msg.contains("denied by permission policy")),
		"expected macro capability denial, got: {result:?}"
	);
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|count| count.get()),
		0,
		"denied macro effect must not dispatch actions"
	);
}

#[tokio::test]
async fn nu_macro_respects_configured_decode_limits() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	std::fs::write(
		temp.path().join("xeno.nu"),
		// Returns 2 effects — should exceed max_effects=1
		"export def go [] { [(xeno effect dispatch action invocation_test_action), (xeno effect dispatch action invocation_test_action)] | xeno effects normalize }",
	)
	.expect("xeno.nu should be writable");

	let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
	let mut editor = Editor::new_scratch();
	editor.set_nu_runtime(Some(runtime));

	// Set decode budget: max_effects=1
	editor.state.config.config.nu = Some(xeno_registry::config::NuConfig {
		budget_macro: Some(xeno_registry::config::DecodeBudgetOverrides {
			max_effects: Some(1),
			..Default::default()
		}),
		budget_hook: None,
		permissions_macro: None,
		permissions_hook: None,
	});

	let result = editor
		.run_invocation(
			Invocation::Nu {
				name: "go".to_string(),
				args: Vec::new(),
			},
			InvocationPolicy::enforcing(),
		)
		.await;

	assert!(
		matches!(result.status, InvocationStatus::CommandError) && result.detail_text().is_some_and(|msg| msg.contains("effect count exceeds")),
		"expected decode limit error, got: {result:?}"
	);
}
