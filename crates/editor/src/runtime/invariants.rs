use std::cell::RefCell;
use std::thread_local;
use std::time::Duration;

use xeno_primitives::{BoxFutureLocal, Key, KeyCode};
use xeno_registry::hooks::HookPriority;

use super::{CursorStyle, RuntimeEvent};
use crate::Editor;
use crate::commands::{CommandError, CommandOutcome, EditorCommandContext};
use crate::runtime::mailbox::DeferredInvocationSource;
use crate::runtime::pump::PumpPhase;
use crate::scheduler::{WorkItem, WorkKind};
use crate::types::{DeferredWorkItem, Invocation};

fn runtime_invariant_test_quit_command<'a>(_ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Ok(CommandOutcome::Quit) })
}

thread_local! {
	static RUNTIME_INVARIANT_RECORDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn runtime_invariant_record_command<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	let value = ctx.args.first().copied().unwrap_or_default().to_string();
	Box::pin(async move {
		RUNTIME_INVARIANT_RECORDS.with(|records| {
			records.borrow_mut().push(value);
		});
		Ok(CommandOutcome::Ok)
	})
}

crate::editor_command!(
	runtime_invariant_test_quit_command,
	{
		description: "Runtime invariant test command that requests quit"
	},
	handler: runtime_invariant_test_quit_command
);

crate::editor_command!(
	runtime_invariant_record_command,
	{
		description: "Runtime invariant test command that records invocation order"
	},
	handler: runtime_invariant_record_command
);

/// Must execute one maintenance `pump` after handling each runtime event.
///
/// * Enforced in: `Editor::on_event`
/// * Failure symptom: input handlers mutate state without advancing deferred work.
#[tokio::test]
async fn test_on_event_implies_single_pump_cycle() {
	let mut editor = Editor::new_scratch();
	let _ = editor.pump().await;

	let directive = editor.on_event(RuntimeEvent::Key(Key::char('i'))).await;
	assert_eq!(directive.poll_timeout, Some(Duration::from_millis(16)));
}

/// Must defer overlay commit execution to `pump` via deferred work queue.
///
/// * Enforced in: `Editor::handle_key_active`, `Editor::pump`
/// * Failure symptom: overlay commit runs re-entrantly inside key handling.
#[tokio::test]
async fn test_overlay_commit_deferred_until_pump() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_key(Key::new(KeyCode::Enter)).await;
	assert!(editor.frame().deferred_work.has_overlay_commit());
	assert!(editor.overlay_kind().is_some());

	let _ = editor.pump().await;
	assert!(!editor.frame().deferred_work.has_overlay_commit());
	assert!(editor.overlay_kind().is_none());
}

/// Must apply at most one deferred overlay commit per pump cycle.
///
/// * Enforced in: `runtime::pump::run_pump_cycle_with_report`
/// * Failure symptom: duplicate commit requests execute repeatedly in one cycle and reorder deferred work.
#[tokio::test]
async fn test_pump_applies_at_most_one_overlay_commit_per_cycle() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	editor.frame_mut().deferred_work.push(DeferredWorkItem::OverlayCommit);
	editor.frame_mut().deferred_work.push(DeferredWorkItem::OverlayCommit);

	let _ = editor.pump().await;

	assert!(editor.overlay_kind().is_none());
	assert!(editor.frame().deferred_work.has_overlay_commit());
}

/// Must default cursor style to Beam in insert mode and Block otherwise.
///
/// * Enforced in: `Editor::derive_cursor_style`
/// * Failure symptom: frontends render incorrect cursor shape for modal state.
#[cfg_attr(test, test)]
pub(crate) fn test_cursor_style_defaults_follow_mode() {
	let mut editor = Editor::new_scratch();
	assert_eq!(editor.derive_cursor_style(), CursorStyle::Block);

	editor.set_mode(xeno_primitives::Mode::Insert);
	assert_eq!(editor.derive_cursor_style(), CursorStyle::Beam);
}

/// Must preserve round phase ordering so maintenance side effects remain deterministic.
///
/// * Enforced in: `runtime::pump::run_pump_cycle_with_report`
/// * Failure symptom: deferred effects execute in unstable order across pump cycles.
#[tokio::test]
async fn test_pump_round_phase_order_is_stable() {
	let mut editor = Editor::new_scratch();
	let (_directive, report) = editor.pump_with_report().await;
	assert!(!report.rounds.is_empty(), "pump should execute at least one round");

	let expected = vec![
		PumpPhase::UiTickAndTick,
		PumpPhase::FilesystemPump,
		PumpPhase::OverlayCommit,
		PumpPhase::DrainMessages,
		PumpPhase::ApplyWorkspaceEdits,
		PumpPhase::KickNuHookEval,
		PumpPhase::DrainScheduler,
		PumpPhase::DrainDeferredInvocations,
	];

	assert_eq!(report.rounds[0].phases, expected);
}

/// Must cap pump maintenance rounds to avoid unbounded single-cycle stall.
///
/// * Enforced in: `runtime::pump::run_pump_cycle_with_report`
/// * Failure symptom: one pump call monopolizes the editor thread under backlog.
#[tokio::test]
async fn test_pump_rounds_are_bounded_by_cap() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(xeno_primitives::Mode::Insert);

	for _ in 0..200 {
		editor.work_scheduler_mut().schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::Hook,
			priority: HookPriority::Interactive,
			doc_id: None,
		});
	}

	tokio::task::yield_now().await;
	let (_directive, report) = editor.pump_with_report().await;

	assert_eq!(report.rounds_executed, super::pump::MAX_PUMP_ROUNDS);
	assert!(report.reached_round_cap);
}

/// Must return an immediate quit directive when drained command or hook work requests quit.
///
/// * Enforced in: `runtime::pump::run_pump_cycle_with_report`
/// * Failure symptom: quit requests wait for later pump cycles and feel laggy.
#[tokio::test]
async fn test_pump_quit_requests_return_immediate_quit_directive() {
	let mut editor = Editor::new_scratch();
	editor.enqueue_deferred_command(
		"runtime_invariant_test_quit_command".to_string(),
		Vec::new(),
		DeferredInvocationSource::CommandOps,
	);

	let directive = editor.pump().await;
	assert!(directive.should_quit);
	assert_eq!(directive.poll_timeout, None);

	let mut via_hook = Editor::new_scratch();
	via_hook.enqueue_nu_deferred_invocation(
		Invocation::editor_command("runtime_invariant_test_quit_command", Vec::new()),
		DeferredInvocationSource::NuHookDispatch,
	);

	let directive = via_hook.pump().await;
	assert!(directive.should_quit);
	assert_eq!(directive.poll_timeout, None);
}

/// Must preserve global FIFO order when draining deferred invocations from mixed sources.
///
/// * Enforced in: `Editor::drain_deferred_invocations_report`
/// * Failure symptom: deferred commands execute out-of-order across queue sources.
#[tokio::test]
async fn test_deferred_invocation_mailbox_preserves_fifo_across_sources() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.enqueue_deferred_command(
		"runtime_invariant_record_command".to_string(),
		vec!["one".to_string()],
		DeferredInvocationSource::Overlay,
	);
	editor.enqueue_deferred_command(
		"runtime_invariant_record_command".to_string(),
		vec!["two".to_string()],
		DeferredInvocationSource::ActionEffect,
	);
	editor.enqueue_nu_deferred_invocation(
		Invocation::command("runtime_invariant_record_command", vec!["three".to_string()]),
		DeferredInvocationSource::NuScheduledMacro,
	);

	let _ = editor.pump().await;
	let recorded = RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow().clone());
	assert_eq!(recorded, vec!["one".to_string(), "two".to_string(), "three".to_string()]);
}

/// Must bound deferred invocation drain count per round to preserve latency.
///
/// * Enforced in: `runtime::pump::phases::phase_drain_deferred_invocations`
/// * Failure symptom: one round drains unbounded invocation backlog and stalls input responsiveness.
#[tokio::test]
async fn test_deferred_invocation_drain_is_bounded_per_round() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	for idx in 0..100 {
		editor.enqueue_deferred_command(
			"runtime_invariant_record_command".to_string(),
			vec![idx.to_string()],
			DeferredInvocationSource::ActionEffect,
		);
	}

	let (_directive, report) = editor.pump_with_report().await;
	assert!(!report.rounds.is_empty());
	assert!(report.rounds.iter().all(|round| round.work.drained_deferred_invocations <= 32));
	assert_eq!(report.rounds[0].work.drained_deferred_invocations, 32);
	assert_eq!(editor.state.invocation_mailbox.len(), 4);
	assert!(report.reached_round_cap);
}

/// Must clear only the targeted Nu stop-scope generation from the deferred mailbox.
///
/// * Enforced in: `Editor::clear_deferred_nu_scope`, `Editor::enqueue_nu_deferred_invocation`
/// * Failure symptom: stop propagation drops unrelated deferred work or fails to drop stale Nu work.
#[tokio::test]
async fn test_nu_stop_scope_clear_is_generation_local() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.enqueue_nu_deferred_invocation(
		Invocation::command("runtime_invariant_record_command", vec!["nu-old".to_string()]),
		DeferredInvocationSource::NuHookDispatch,
	);
	editor.enqueue_deferred_command(
		"runtime_invariant_record_command".to_string(),
		vec!["global".to_string()],
		DeferredInvocationSource::Overlay,
	);

	let cleared_generation = editor.state.nu.advance_stop_scope_generation();
	editor.clear_deferred_nu_scope(cleared_generation);

	editor.enqueue_nu_deferred_invocation(
		Invocation::command("runtime_invariant_record_command", vec!["nu-new".to_string()]),
		DeferredInvocationSource::NuHookDispatch,
	);

	let _ = editor.pump().await;
	let recorded = RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow().clone());
	assert_eq!(recorded, vec!["global".to_string(), "nu-new".to_string()]);
}
