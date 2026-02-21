use xeno_invocation::CommandRoute;
use xeno_primitives::range::CharIdx;
use xeno_primitives::{Mode, Selection};
use xeno_registry::actions::editor_ctx::{
	CursorAccess, DeferredInvocationAccess, EditorCapabilities, HandleOutcome, ModeAccess, NotificationAccess, SelectionAccess,
};
use xeno_registry::actions::{ActionEffects, ActionResult, AppEffect, DeferredInvocationKind, DeferredInvocationRequest, UiEffect, ViewEffect};
use xeno_registry::notifications::{Notification, keys};

use super::apply_effects;
use crate::Editor;
use crate::runtime::work_queue::{RuntimeWorkKind, RuntimeWorkSource, WorkExecutionPolicy};
use crate::types::Invocation;

struct MockEditor {
	cursor: CharIdx,
	selection: Selection,
	mode: Mode,
	notifications: Vec<Notification>,
	deferred_requests: Vec<DeferredInvocationRequest>,
	effect_log: Vec<String>,
}

impl MockEditor {
	fn new() -> Self {
		Self {
			cursor: CharIdx::from(0usize),
			selection: Selection::point(CharIdx::from(0usize)),
			mode: Mode::Normal,
			notifications: Vec::new(),
			deferred_requests: Vec::new(),
			effect_log: Vec::new(),
		}
	}

	fn push_log(&mut self, entry: impl Into<String>) {
		self.effect_log.push(entry.into());
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
		self.push_log(format!("set_cursor:{pos}"));
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
		self.push_log(format!("set_selection:{}", sel.primary().head));
		self.selection = sel;
	}
}

impl ModeAccess for MockEditor {
	fn mode(&self) -> Mode {
		self.mode.clone()
	}

	fn set_mode(&mut self, mode: Mode) {
		self.push_log(format!("set_mode:{mode:?}"));
		self.mode = mode;
	}
}

impl NotificationAccess for MockEditor {
	fn emit(&mut self, notification: Notification) {
		self.push_log(format!("notify:{}", notification.id));
		self.notifications.push(notification);
	}

	fn clear_notifications(&mut self) {
		self.notifications.clear();
	}
}

impl DeferredInvocationAccess for MockEditor {
	fn queue_invocation(&mut self, request: DeferredInvocationRequest) {
		match &request.kind {
			DeferredInvocationKind::Command { name, .. } => self.push_log(format!("queue_invocation:command:{name}")),
			DeferredInvocationKind::EditorCommand { name, .. } => self.push_log(format!("queue_invocation:editor_command:{name}")),
		}
		self.deferred_requests.push(request);
	}
}

impl EditorCapabilities for MockEditor {
	fn deferred_invocations(&mut self) -> Option<&mut dyn DeferredInvocationAccess> {
		Some(self)
	}
}

/// Must keep effects interpreter capability-honest and editor-agnostic.
///
/// * Enforced in: `editor_ctx::apply_effects`
/// * Failure symptom: registry effects require concrete `Editor` downcasts.
#[cfg_attr(test, test)]
pub fn test_honesty_rule() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);
	let outcome = apply_effects(&ActionEffects::cursor(CharIdx::from(4usize)), &mut ctx, false);
	assert_eq!(outcome, HandleOutcome::Handled);
	assert_eq!(editor.cursor, CharIdx::from(4usize));
}

/// Must apply mixed view/ui/app effect sequences in strict list order.
///
/// * Enforced in: `editor_ctx::apply_effects`
/// * Failure symptom: mode/notify/deferred operations observe unstable ordering.
#[cfg_attr(test, test)]
pub fn test_multi_effect_sequences_apply_in_strict_order() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);
	let deferred = DeferredInvocationRequest::command("invariant_order".to_string(), Vec::new());

	let outcome = apply_effects(
		&ActionEffects::new()
			.with(ViewEffect::SetCursor(CharIdx::from(9usize)))
			.with(UiEffect::Notify(keys::info("editor-ctx-order")))
			.with(AppEffect::SetMode(Mode::Insert))
			.with(AppEffect::QueueInvocation(deferred.clone())),
		&mut ctx,
		false,
	);

	assert_eq!(outcome, HandleOutcome::Handled);
	assert_eq!(editor.cursor, CharIdx::from(9usize));
	assert_eq!(editor.mode, Mode::Insert);
	assert_eq!(editor.deferred_requests, vec![deferred]);

	let cursor_idx = editor
		.effect_log
		.iter()
		.position(|entry| entry == "set_cursor:9")
		.expect("cursor effect should be applied");
	let notify_idx = editor
		.effect_log
		.iter()
		.position(|entry| entry == "notify:xeno-registry::info")
		.expect("notify effect should be applied");
	let mode_idx = editor
		.effect_log
		.iter()
		.position(|entry| entry == "set_mode:Insert")
		.expect("mode effect should be applied");
	let deferred_idx = editor
		.effect_log
		.iter()
		.position(|entry| entry == "queue_invocation:command:invariant_order")
		.expect("deferred invocation should be queued");

	assert!(
		cursor_idx < notify_idx && notify_idx < mode_idx && mode_idx < deferred_idx,
		"mixed effects must apply in list order"
	);
}

/// Must route side effects through capability providers and reveal them only through sink flush.
///
/// * Enforced in: `EditorCaps` capability impls, `Editor::flush_effects`
/// * Failure symptom: notifications/runtime invocations appear before explicit sink flush.
#[tokio::test(flavor = "current_thread")]
pub async fn test_side_effects_route_through_capability_provider_and_sink_path() {
	let mut editor = Editor::new_scratch();
	let effects = ActionEffects::new()
		.with(UiEffect::Notify(keys::info("editor-ctx-sink-route")))
		.with(AppEffect::QueueInvocation(DeferredInvocationRequest::command(
			"side_effect_route_probe".to_string(),
			Vec::new(),
		)));

	{
		let mut caps = editor.caps();
		let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut caps);
		let outcome = apply_effects(&effects, &mut ctx, false);
		assert_eq!(outcome, HandleOutcome::Handled);
	}

	assert!(editor.state.ui.notifications.take_pending().is_empty());
	assert_eq!(editor.runtime_work_len(), 0);

	editor.flush_effects();

	let notifications = editor.state.ui.notifications.take_pending();
	assert_eq!(notifications.len(), 1);
	assert_eq!(notifications[0].id, keys::info("editor-ctx-sink-route").id);

	let queued = editor.runtime_work_snapshot();
	assert_eq!(queued.len(), 1);
	let RuntimeWorkKind::Invocation(queued_invocation) = &queued[0].kind else {
		panic!("queued side effect should produce invocation runtime work");
	};
	assert_eq!(queued_invocation.source, RuntimeWorkSource::ActionEffect);
	assert_eq!(queued_invocation.execution, WorkExecutionPolicy::LogOnlyCommandPath);
}

/// Must route action result effects through `apply_effects` and defer sink consequences until flush.
///
/// * Enforced in: `Editor::apply_action_result`, `editor_ctx::apply_effects`, `Editor::flush_effects`
/// * Failure symptom: invocation boundary bypasses interpreter or leaks side effects pre-flush.
#[tokio::test(flavor = "current_thread")]
pub async fn test_action_result_effects_enter_apply_effects_and_defer_until_sink_flush() {
	let mut editor = Editor::new_scratch();
	let effects = ActionEffects::new()
		.with(ViewEffect::SetCursor(CharIdx::from(12usize)))
		.with(UiEffect::Notify(keys::info("editor-ctx-apply-effects")))
		.with(AppEffect::SetMode(Mode::Insert))
		.with(AppEffect::QueueInvocation(DeferredInvocationRequest::editor_command(
			"stats".to_string(),
			Vec::new(),
		)));

	let should_quit = editor.apply_action_result("editor_ctx_invariant_effect_boundary", ActionResult::Effects(effects), false);
	assert!(!should_quit);
	assert_eq!(editor.buffer().cursor, CharIdx::from(12usize));
	assert_eq!(editor.mode(), Mode::Insert);
	assert!(editor.state.ui.notifications.take_pending().is_empty());
	assert_eq!(editor.runtime_work_len(), 0);

	editor.flush_effects();

	let notifications = editor.state.ui.notifications.take_pending();
	assert_eq!(notifications.len(), 1);
	assert_eq!(notifications[0].id, keys::info("editor-ctx-apply-effects").id);

	let queued = editor.runtime_work_snapshot();
	assert_eq!(queued.len(), 1);
	let RuntimeWorkKind::Invocation(queued_invocation) = &queued[0].kind else {
		panic!("action-result deferred consequence should queue runtime invocation");
	};
	assert_eq!(queued_invocation.source, RuntimeWorkSource::ActionEffect);
	assert_eq!(queued_invocation.execution, WorkExecutionPolicy::LogOnlyCommandPath);
	assert!(matches!(
		&queued_invocation.invocation,
		Invocation::Command(command) if command.name == "stats" && command.route == CommandRoute::Editor
	));
}

