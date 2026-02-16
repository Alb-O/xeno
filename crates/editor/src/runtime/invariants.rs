use std::time::Duration;

use xeno_primitives::{BoxFutureLocal, Key, KeyCode};
use xeno_registry::hooks::HookPriority;

use super::{CursorStyle, RuntimeEvent};
use crate::Editor;
use crate::commands::{CommandError, CommandOutcome, EditorCommandContext};
use crate::runtime::pump::PumpPhase;
use crate::scheduler::{WorkItem, WorkKind};
use crate::types::{DeferredWorkItem, Invocation};

fn runtime_invariant_test_quit_command<'a>(_ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Ok(CommandOutcome::Quit) })
}

crate::editor_command!(
	runtime_invariant_test_quit_command,
	{
		description: "Runtime invariant test command that requests quit"
	},
	handler: runtime_invariant_test_quit_command
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
		PumpPhase::DrainCommandQueue,
		PumpPhase::DrainNuHookInvocations,
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
	editor
		.state
		.core
		.workspace
		.command_queue
		.push("runtime_invariant_test_quit_command", Vec::new());

	let directive = editor.pump().await;
	assert!(directive.should_quit);
	assert_eq!(directive.poll_timeout, None);

	let mut via_hook = Editor::new_scratch();
	via_hook
		.state
		.nu
		.extend_pending_hook_invocations(vec![Invocation::editor_command("runtime_invariant_test_quit_command", Vec::new())]);

	let directive = via_hook.pump().await;
	assert!(directive.should_quit);
	assert_eq!(directive.poll_timeout, None);
}
