use std::cell::Cell;

use xeno_primitives::range::CharIdx;
use xeno_primitives::{BoxFutureLocal, Mode, Selection};
use xeno_registry::{
	ActionEffects, ActionResult, Capability, CommandContext, CommandError, CommandOutcome,
	CursorAccess, EditorCapabilities, HookAction, HookEventData, ModeAccess, Notification,
	NotificationAccess, SelectionAccess, action, command, hook,
};

use super::*;

thread_local! {
	static ACTION_PRE_COUNT: Cell<usize> = const { Cell::new(0) };
	static ACTION_POST_COUNT: Cell<usize> = const { Cell::new(0) };
}

action!(
	invocation_test_action,
	{ description: "Invocation test action" },
	|_ctx| ActionResult::Effects(ActionEffects::ok())
);

action!(
	invocation_edit_action,
	{
		description: "Invocation edit action",
		caps: &[Capability::Edit]
	},
	|_ctx| ActionResult::Effects(ActionEffects::ok())
);

hook!(
	invocation_test_action_pre,
	ActionPre,
	0,
	"Count action pre hooks",
	|ctx| {
		if let HookEventData::ActionPre { .. } = &ctx.data {
			ACTION_PRE_COUNT.with(|count| count.set(count.get() + 1));
		}
		HookAction::done()
	}
);

hook!(
	invocation_test_action_post,
	ActionPost,
	0,
	"Count action post hooks",
	|ctx| {
		if let HookEventData::ActionPost { .. } = &ctx.data {
			ACTION_POST_COUNT.with(|count| count.set(count.get() + 1));
		}
		HookAction::done()
	}
);

fn invocation_test_command_fail<'a>(
	_ctx: &'a mut CommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Err(CommandError::Failed("boom".into())) })
}

command!(
	invocation_test_command_fail,
	{ description: "Invocation test command failure" },
	handler: invocation_test_command_fail
);

fn register_invocation_test_defs() {
	xeno_registry::actions::register_action(&ACTION_invocation_test_action);
	xeno_registry::actions::register_action(&ACTION_invocation_edit_action);
	xeno_registry::commands::register_command(&CMD_invocation_test_command_fail);
	xeno_registry::hooks::register_hook(&HOOK_invocation_test_action_pre);
	xeno_registry::hooks::register_hook(&HOOK_invocation_test_action_post);
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
	register_invocation_test_defs();
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
	register_invocation_test_defs();
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
	register_invocation_test_defs();
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
	register_invocation_test_defs();
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
