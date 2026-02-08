use std::cell::Cell;

use xeno_primitives::range::CharIdx;
use xeno_primitives::{BoxFutureLocal, Mode, Selection};
use xeno_registry::hooks::{
	HookAction, HookContext, HookDef, HookHandler, HookMutability, HookPriority,
};
use xeno_registry::{
	ActionEffects, ActionResult, Capability, CommandContext, CommandError, CommandOutcome,
	CursorAccess, EditorCapabilities, ModeAccess, Notification, NotificationAccess,
	SelectionAccess,
};

use super::*;

thread_local! {
	static ACTION_PRE_COUNT: Cell<usize> = const { Cell::new(0) };
	static ACTION_POST_COUNT: Cell<usize> = const { Cell::new(0) };
}

fn handler_invocation_test_action(_ctx: &xeno_registry::ActionContext) -> ActionResult {
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_INVOCATION_TEST: xeno_registry::ActionDef = xeno_registry::ActionDef {
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

fn handler_invocation_edit_action(_ctx: &xeno_registry::ActionContext) -> ActionResult {
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_INVOCATION_EDIT: xeno_registry::ActionDef = xeno_registry::ActionDef {
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

fn invocation_test_command_fail<'a>(
	_ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Err(CommandError::Failed("boom".into())) })
}

static CMD_TEST_FAIL: xeno_registry::CommandDef = xeno_registry::CommandDef {
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

fn register_invocation_test_plugin(
	db: &mut xeno_registry::db::builder::RegistryDbBuilder,
) -> Result<(), xeno_registry::RegistryError> {
	db.register_action(&ACTION_INVOCATION_TEST);
	db.register_action(&ACTION_INVOCATION_EDIT);
	db.register_command(&CMD_TEST_FAIL);
	db.register_hook(&HOOK_ACTION_PRE);
	db.register_hook(&HOOK_ACTION_POST);
	Ok(())
}

inventory::submit! {
	xeno_registry::PluginDef::new(
		xeno_registry::RegistryMetaStatic::minimal(
			"invocation-test",
			"Invocation Test",
			"Test defs for invocation tests",
		),
		register_invocation_test_plugin,
	)
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
	assert_eq!(
		Invocation::action("move_left").describe(),
		"action:move_left"
	);
	assert_eq!(
		Invocation::action_with_count("move_down", 5).describe(),
		"action:move_downx5"
	);
	assert_eq!(
		Invocation::command("write", vec!["file.txt".into()]).describe(),
		"cmd:write file.txt"
	);
	assert_eq!(
		Invocation::editor_command("quit", vec![]).describe(),
		"editor_cmd:quit"
	);
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
	let error = ctx
		.check_all_capabilities(&[Capability::Search])
		.expect_err("expected missing capability");

	let notified = Cell::new(false);
	let logged = Cell::new(false);

	let result = handle_capability_violation(
		InvocationPolicy::enforcing(),
		error,
		|_err| notified.set(true),
		|_err| logged.set(true),
	);

	assert!(notified.get());
	assert!(!logged.get());
	assert!(matches!(
		result,
		Some(InvocationResult::CapabilityDenied(Capability::Search))
	));
}

#[test]
fn capability_enforcement_logs_in_log_only_mode() {
	let mut editor = MockEditor::new();
	let mut ctx = EditorContext::new(&mut editor);
	let error = ctx
		.check_all_capabilities(&[Capability::Search])
		.expect_err("expected missing capability");

	let notified = Cell::new(false);
	let logged = Cell::new(false);

	let result = handle_capability_violation(
		InvocationPolicy::log_only(),
		error,
		|_err| notified.set(true),
		|_err| logged.set(true),
	);

	assert!(result.is_none());
	assert!(!notified.get());
	assert!(logged.get());
}

#[test]
fn action_hooks_fire_once() {
	// Test defs registered via inventory::submit!(PluginDef) at DB init time.
	ACTION_PRE_COUNT.with(|count| count.set(0));
	ACTION_POST_COUNT.with(|count| count.set(0));

	let mut editor = Editor::new_scratch();
	let result = editor.invoke_action("invocation_test_action", 1, false, None, None);
	assert!(matches!(result, InvocationResult::Ok));

	let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
	let post_count = ACTION_POST_COUNT.with(|count| count.get());

	assert_eq!(pre_count, 1);
	assert_eq!(post_count, 1);
}

#[test]
fn readonly_enforcement_blocks_edit_actions() {
	// Test defs registered via inventory::submit!(PluginDef) at DB init time.
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let result = editor.run_action_invocation(
		"invocation_edit_action",
		1,
		false,
		None,
		None,
		InvocationPolicy::enforcing(),
	);

	assert!(matches!(result, InvocationResult::ReadonlyDenied));
}

#[test]
fn readonly_disabled_allows_edit_actions() {
	// Test defs registered via inventory::submit!(PluginDef) at DB init time.
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let result = editor.run_action_invocation(
		"invocation_edit_action",
		1,
		false,
		None,
		None,
		InvocationPolicy::log_only(),
	);

	assert!(matches!(result, InvocationResult::Ok));
}

#[test]
fn command_error_propagates() {
	// Test defs registered via inventory::submit!(PluginDef) at DB init time.
	let mut editor = Editor::new_scratch();
	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();
	let result = rt.block_on(editor.run_command_invocation(
		"invocation_test_command_fail",
		Vec::new(),
		InvocationPolicy::enforcing(),
	));

	assert!(matches!(
		result,
		InvocationResult::CommandError(msg) if msg.contains("boom")
	));
}
