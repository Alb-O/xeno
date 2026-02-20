use std::cell::Cell;
use std::collections::HashSet;

use xeno_primitives::range::CharIdx;
use xeno_primitives::{BoxFutureLocal, Key, KeyCode, Mode, Selection};
use xeno_registry::actions::{ActionEffects, ActionResult, CursorAccess, EditorCapabilities, ModeAccess, NotificationAccess, SelectionAccess};
use xeno_registry::commands::{CommandContext, CommandOutcome};
use xeno_registry::hooks::{HookAction, HookContext, HookDef, HookHandler, HookMutability, HookPriority};
use xeno_registry::notifications::Notification;
use xeno_registry::{Capability, CommandError};

use super::*;
use crate::types::{InvocationOutcome, InvocationStatus, InvocationTarget};

thread_local! {
	static ACTION_PRE_COUNT: Cell<usize> = const { Cell::new(0) };
	static ACTION_POST_COUNT: Cell<usize> = const { Cell::new(0) };
	static INVOCATION_TEST_ACTION_COUNT: Cell<usize> = const { Cell::new(0) };
	static INVOCATION_TEST_ACTION_ALT_COUNT: Cell<usize> = const { Cell::new(0) };
}

fn handler_invocation_test_action(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(c.get() + 1));
	ActionResult::Effects(ActionEffects::ok())
}

fn handler_invocation_test_action_alt(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	INVOCATION_TEST_ACTION_ALT_COUNT.with(|c| c.set(c.get() + 1));
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_INVOCATION_TEST: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action",
		name: "invocation_test_action",
		keys: &[],
		description: "Invocation test action",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		required_caps: &[],
		flags: 0,
	},
	short_desc: "Invocation test action",
	handler: handler_invocation_test_action,
	bindings: &[],
};

static ACTION_INVOCATION_TEST_ALT: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action_alt",
		name: "invocation_test_action_alt",
		keys: &[],
		description: "Invocation test action alt",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		required_caps: &[],
		flags: 0,
	},
	short_desc: "Invocation test action alt",
	handler: handler_invocation_test_action_alt,
	bindings: &[],
};

fn handler_invocation_edit_action(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_INVOCATION_EDIT: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_edit_action",
		name: "invocation_edit_action",
		keys: &[],
		description: "Invocation edit action",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		required_caps: &[Capability::Edit],
		flags: 0,
	},
	short_desc: "Invocation edit action",
	handler: handler_invocation_edit_action,
	bindings: &[],
};

fn hook_handler_action_pre(ctx: &HookContext) -> HookAction {
	if let xeno_registry::HookEventData::ActionPre { .. } = &ctx.data {
		ACTION_PRE_COUNT.with(|count| count.set(count.get() + 1));
	}
	HookAction::done()
}

static HOOK_ACTION_PRE: HookDef = HookDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action_pre",
		name: "invocation_test_action_pre",
		keys: &[],
		description: "Count action pre hooks",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		required_caps: &[],
		flags: 0,
	},
	event: xeno_registry::HookEvent::ActionPre,
	mutability: HookMutability::Immutable,
	execution_priority: HookPriority::Interactive,
	handler: HookHandler::Immutable(hook_handler_action_pre),
};

fn hook_handler_action_post(ctx: &HookContext) -> HookAction {
	if let xeno_registry::HookEventData::ActionPost { .. } = &ctx.data {
		ACTION_POST_COUNT.with(|count| count.set(count.get() + 1));
	}
	HookAction::done()
}

static HOOK_ACTION_POST: HookDef = HookDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action_post",
		name: "invocation_test_action_post",
		keys: &[],
		description: "Count action post hooks",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		required_caps: &[],
		flags: 0,
	},
	event: xeno_registry::HookEvent::ActionPost,
	mutability: HookMutability::Immutable,
	execution_priority: HookPriority::Interactive,
	handler: HookHandler::Immutable(hook_handler_action_post),
};

fn invocation_test_command_fail<'a>(_ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Err(CommandError::Failed("boom".into())) })
}

static CMD_TEST_FAIL: xeno_registry::commands::CommandDef = xeno_registry::commands::CommandDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_command_fail",
		name: "invocation_test_command_fail",
		keys: &[],
		description: "Invocation test command failure",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		required_caps: &[],
		flags: 0,
	},
	handler: invocation_test_command_fail,
	user_data: None,
};

fn register_invocation_test_defs(db: &mut xeno_registry::db::builder::RegistryDbBuilder) -> Result<(), xeno_registry::db::builder::RegistryError> {
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_INVOCATION_TEST.clone()));
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_INVOCATION_TEST_ALT.clone()));
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_INVOCATION_EDIT.clone()));
	db.push_domain::<xeno_registry::commands::Commands>(xeno_registry::commands::def::CommandInput::Static(CMD_TEST_FAIL.clone()));
	db.push_domain::<xeno_registry::hooks::Hooks>(xeno_registry::hooks::HookInput::Static(HOOK_ACTION_PRE));
	db.push_domain::<xeno_registry::hooks::Hooks>(xeno_registry::hooks::HookInput::Static(HOOK_ACTION_POST));
	Ok(())
}

inventory::submit! {
	xeno_registry::db::builtins::BuiltinsReg {
		ordinal: 65000,
		f: register_invocation_test_defs,
	}
}

struct MockEditor {
	cursor: CharIdx,
	selection: Selection,
	mode: Mode,
	notifications: Vec<Notification>,
}

impl MockEditor {
	fn new() -> Self {
		Self {
			cursor: CharIdx::from(0usize),
			selection: Selection::point(CharIdx::from(0usize)),
			mode: Mode::Normal,
			notifications: Vec::new(),
		}
	}
}

impl CursorAccess for MockEditor {
	fn focused_view(&self) -> xeno_registry::hooks::ViewId {
		xeno_registry::hooks::ViewId::text(1)
	}

	fn cursor(&self) -> CharIdx {
		self.cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		Some((0, self.cursor))
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		self.cursor = pos;
	}
}

impl SelectionAccess for MockEditor {
	fn selection(&self) -> &Selection {
		&self.selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.selection
	}

	fn set_selection(&mut self, sel: Selection) {
		self.selection = sel;
	}
}

impl ModeAccess for MockEditor {
	fn mode(&self) -> Mode {
		self.mode.clone()
	}

	fn set_mode(&mut self, mode: Mode) {
		self.mode = mode;
	}
}

impl NotificationAccess for MockEditor {
	fn emit(&mut self, notification: Notification) {
		self.notifications.push(notification);
	}

	fn clear_notifications(&mut self) {
		self.notifications.clear();
	}
}

impl EditorCapabilities for MockEditor {}

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
	assert!(!policy.enforce_caps);
	assert!(!policy.enforce_readonly);

	let policy = InvocationPolicy::enforcing();
	assert!(policy.enforce_caps);
	assert!(policy.enforce_readonly);
}

#[test]
fn capability_enforcement_blocks_when_enforced() {
	let mut editor = MockEditor::new();
	let mut ctx = EditorContext::new(&mut editor);
	let error = ctx.check_all_capabilities(&[Capability::Search]).expect_err("expected missing capability");

	let notified = Cell::new(false);
	let logged = Cell::new(false);

	let result = handle_capability_violation(
		policy_gate::InvocationKind::Command,
		InvocationPolicy::enforcing(),
		error,
		|_err| notified.set(true),
		|_err| logged.set(true),
	);

	assert!(notified.get());
	assert!(!logged.get());
	assert!(matches!(
		&result,
		Some(InvocationOutcome {
			status: InvocationStatus::CapabilityDenied,
			..
		})
	));
	assert_eq!(result.and_then(|outcome| outcome.denied_capability()), Some(Capability::Search));
}

#[test]
fn capability_enforcement_logs_in_log_only_mode() {
	let mut editor = MockEditor::new();
	let mut ctx = EditorContext::new(&mut editor);
	let error = ctx.check_all_capabilities(&[Capability::Search]).expect_err("expected missing capability");

	let notified = Cell::new(false);
	let logged = Cell::new(false);

	let result = handle_capability_violation(
		policy_gate::InvocationKind::Command,
		InvocationPolicy::log_only(),
		error,
		|_err| notified.set(true),
		|_err| logged.set(true),
	);

	assert!(result.is_none());
	assert!(!notified.get());
	assert!(logged.get());
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
		capabilities_macro: Some(HashSet::new()),
		capabilities_hook: None,
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
		matches!(result.status, InvocationStatus::CommandError) && result.detail_text().is_some_and(|msg| msg.contains("denied by capability policy")),
		"expected macro capability denial, got: {result:?}"
	);
	assert_eq!(
		INVOCATION_TEST_ACTION_COUNT.with(|count| count.get()),
		0,
		"denied macro effect must not dispatch actions"
	);
}

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
		capabilities_macro: None,
		capabilities_hook: Some(HashSet::new()),
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
		.integration.nu
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
		capabilities_macro: None,
		capabilities_hook: None,
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
