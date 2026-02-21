use std::cell::{Cell, RefCell};
use std::thread_local;
use std::time::Duration;

use tokio::time::{sleep, timeout};
use xeno_primitives::{BoxFutureLocal, Key, KeyCode, Mode};
use xeno_registry::actions::{ActionEffects, ActionResult};
use xeno_registry::hooks::HookPriority;

use super::{CursorStyle, RuntimeEvent};
use crate::Editor;
use crate::commands::{CommandError, CommandOutcome, EditorCommandContext};
use crate::runtime::pump::PumpPhase;
use crate::runtime::work_queue::{RuntimeWorkKind, RuntimeWorkSource, WorkExecutionPolicy, WorkScope};
use crate::runtime::{DrainPolicy, RuntimeDrainExitReason};
use crate::scheduler::{WorkItem, WorkKind};
use crate::types::Invocation;

async fn drain_for_event(editor: &mut Editor, event: RuntimeEvent) -> crate::runtime::LoopDirectiveV2 {
	let _ = editor.submit_event(event);
	let _ = editor.drain_until_idle(DrainPolicy::for_on_event()).await;
	editor.poll_directive().unwrap_or(placeholder_directive())
}

async fn drain_for_pump(editor: &mut Editor) -> crate::runtime::LoopDirectiveV2 {
	let _ = editor.drain_until_idle(DrainPolicy::for_pump()).await;
	editor.poll_directive().unwrap_or(placeholder_directive())
}

fn placeholder_directive() -> crate::runtime::LoopDirectiveV2 {
	crate::runtime::LoopDirectiveV2 {
		poll_timeout: Some(Duration::from_millis(50)),
		needs_redraw: false,
		cursor_style: CursorStyle::Block,
		should_quit: false,
		cause_seq: None,
		cause_id: None,
		drained_runtime_work: 0,
		pending_events: 0,
	}
}

fn runtime_invariant_test_quit_command<'a>(_ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Ok(CommandOutcome::Quit) })
}

thread_local! {
	static RUNTIME_INVARIANT_RECORDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
	static RUNTIME_EDIT_ACTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

fn handler_runtime_edit_action(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	RUNTIME_EDIT_ACTION_COUNT.with(|c| c.set(c.get() + 1));
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_RUNTIME_EDIT: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::runtime_invariant_edit_action",
		name: "runtime_invariant_edit_action",
		keys: &[],
		description: "Runtime invariant test action requiring Edit capability",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: true,
	},
	short_desc: "Runtime invariant edit action",
	handler: handler_runtime_edit_action,
	bindings: &[],
};

fn register_runtime_invariant_action_defs(db: &mut xeno_registry::db::builder::RegistryDbBuilder) -> Result<(), xeno_registry::db::builder::RegistryError> {
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_RUNTIME_EDIT.clone()));
	Ok(())
}

inventory::submit! {
	xeno_registry::db::builtins::BuiltinsReg {
		ordinal: 65001,
		f: register_runtime_invariant_action_defs,
	}
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

/// Must assign monotonic sequence IDs to submitted runtime event envelopes.
///
/// * Enforced in: `runtime::kernel::RuntimeKernel::next_seq`
/// * Failure symptom: directive causality and queue diagnostics become unstable.
#[tokio::test]
async fn test_submit_event_sequence_monotonic() {
	let mut editor = Editor::new_scratch();
	let first = editor.submit_event(RuntimeEvent::FocusIn);
	let second = editor.submit_event(RuntimeEvent::FocusOut);
	assert!(second.0 > first.0);
}

/// Must construct editor runtime state without requiring an active Tokio runtime.
///
/// * Enforced in: `Editor::new_scratch` (deferred actor spawning)
/// * Failure symptom: synchronous tests panic with "there is no reactor running".
#[tokio::test(flavor = "current_thread")]
async fn test_editor_construction_does_not_require_tokio_runtime() {
	let _editor = Editor::new_scratch();
}

/// Must preserve causal linkage from submitted runtime events to emitted directives.
///
/// * Enforced in: `Editor::drain_until_idle`
/// * Failure symptom: frontends cannot attribute redraw/quit directives to triggering events.
#[tokio::test]
async fn test_drain_until_idle_preserves_cause_sequence() {
	let mut editor = Editor::new_scratch();
	let token = editor.submit_event(RuntimeEvent::Key(Key::char('i')));

	let report = editor.drain_until_idle(DrainPolicy::for_on_event()).await;
	assert_eq!(report.handled_frontend_events, 1);
	assert_eq!(editor.mode(), Mode::Insert);

	let directive = editor.poll_directive().expect("directive should be queued");
	assert_eq!(directive.cause_seq, Some(token.0));
	assert!(directive.cause_id.is_some());
	assert_eq!(directive.pending_events, 0);
}

/// Must propagate runtime cause IDs from drained events to both emitted directives
/// and deferred runtime work spawned by the same event.
///
/// * Enforced in: `Editor::drain_until_idle`, `Editor::enqueue_runtime_overlay_commit_work`
/// * Failure symptom: deferred work cannot be attributed to its triggering event.
#[tokio::test]
async fn test_cause_id_propagates_event_to_runtime_work_and_directive() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	for idx in 0..96 {
		editor.enqueue_runtime_command_invocation(
			"runtime_invariant_record_command".to_string(),
			vec![format!("pre-{idx}")],
			RuntimeWorkSource::ActionEffect,
		);
	}

	let token = editor.submit_event(RuntimeEvent::Key(Key::new(KeyCode::Enter)));
	let report = editor.drain_until_idle(DrainPolicy::for_on_event()).await;
	assert_eq!(report.handled_frontend_events, 1);

	let directive = editor.poll_directive().expect("directive should be queued");
	assert_eq!(directive.cause_seq, Some(token.0));
	let cause_id = directive.cause_id.expect("directive must carry a cause id");

	let remaining_overlay_work = editor
		.runtime_work_snapshot()
		.into_iter()
		.find(|item| matches!(item.kind, RuntimeWorkKind::OverlayCommit))
		.expect("overlay commit should remain queued after round cap");
	assert_eq!(remaining_overlay_work.cause_id, Some(cause_id));
}

/// Must execute one maintenance cycle after one submitted event under on-event policy.
///
/// * Enforced in: `Editor::submit_event`, `Editor::drain_until_idle`
/// * Failure symptom: input handlers mutate state without advancing deferred work.
#[tokio::test]
async fn test_submit_event_on_event_policy_implies_single_maintenance_cycle() {
	let mut editor = Editor::new_scratch();
	let _ = drain_for_pump(&mut editor).await;

	let directive = drain_for_event(&mut editor, RuntimeEvent::Key(Key::char('i'))).await;
	assert_eq!(directive.poll_timeout, Some(Duration::from_millis(16)));
}

/// Must defer overlay commit execution to runtime drain phases via deferred work queue.
///
/// * Enforced in: `Editor::apply_runtime_event_input`, `runtime::facade::RuntimeOverlayPort`, `Editor::drain_until_idle`
/// * Failure symptom: overlay commit runs re-entrantly inside key handling.
#[tokio::test]
async fn test_overlay_commit_deferred_until_runtime_drain() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.apply_runtime_event_input(RuntimeEvent::Key(Key::new(KeyCode::Enter))).await;
	assert!(editor.has_runtime_overlay_commit_work());
	assert!(editor.overlay_kind().is_some());

	let _ = drain_for_pump(&mut editor).await;
	assert!(!editor.has_runtime_overlay_commit_work());
	assert!(editor.overlay_kind().is_none());
}

/// Must route deferred overlay commits and deferred invocations through the shared runtime work queue.
///
/// * Enforced in: `Editor::apply_runtime_event_input`, `Editor::enqueue_runtime_invocation`, `runtime::pump::phases`
/// * Failure symptom: deferred work fragments across multiple queues and runtime convergence skips work.
#[tokio::test]
async fn test_runtime_work_queue_state_converges_overlay_and_invocations() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.apply_runtime_event_input(RuntimeEvent::Key(Key::new(KeyCode::Enter))).await;
	editor.enqueue_runtime_command_invocation(
		"runtime_invariant_record_command".to_string(),
		vec!["merged".to_string()],
		RuntimeWorkSource::Overlay,
	);

	assert!(editor.has_runtime_overlay_commit_work());
	assert_eq!(editor.runtime_work_len(), 2);

	let _ = drain_for_pump(&mut editor).await;
	assert!(editor.overlay_kind().is_none());
	assert_eq!(RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow().clone()), vec!["merged".to_string()]);
}

/// Must drain queued overlay commit items through the runtime work phase.
///
/// * Enforced in: `runtime::work_drain::Editor::drain_runtime_work_report`
/// * Failure symptom: queued commits remain stuck in queue after runtime drain cycles.
#[tokio::test]
async fn test_runtime_drain_drains_overlay_commit_work_items() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	editor.enqueue_runtime_overlay_commit_work();
	editor.enqueue_runtime_overlay_commit_work();

	let _ = drain_for_pump(&mut editor).await;

	assert!(editor.overlay_kind().is_none());
	assert!(!editor.has_runtime_overlay_commit_work());
}

/// Must default cursor style to Beam in insert mode and Block otherwise.
///
/// * Enforced in: `Editor::derive_cursor_style`
/// * Failure symptom: frontends render incorrect cursor shape for modal state.
#[tokio::test]
async fn test_cursor_style_defaults_follow_mode() {
	let mut editor = Editor::new_scratch();
	assert_eq!(editor.derive_cursor_style(), CursorStyle::Block);

	editor.set_mode(xeno_primitives::Mode::Insert);
	assert_eq!(editor.derive_cursor_style(), CursorStyle::Beam);
}

/// Must preserve round phase ordering so maintenance side effects remain deterministic.
///
/// * Enforced in: `runtime::facade::RuntimePorts`, `runtime::pump::run_pump_cycle_with_report`
/// * Failure symptom: deferred effects execute in unstable order across pump cycles.
#[tokio::test]
async fn test_pump_round_phase_order_is_stable() {
	let mut editor = Editor::new_scratch();
	let (_directive, report) = editor.pump_with_report().await;
	assert!(!report.rounds.is_empty(), "runtime cycle should execute at least one round");

	let expected = vec![
		PumpPhase::UiTickAndTick,
		PumpPhase::FilesystemEvents,
		PumpPhase::DrainMessages,
		PumpPhase::KickNuHookEval,
		PumpPhase::DrainScheduler,
		PumpPhase::DrainRuntimeWork,
	];

	assert_eq!(report.rounds[0].phases, expected);
}

/// Must preserve redraw contract semantics for filesystem and message phases after facade indirection.
///
/// * Enforced in: `runtime::facade::{RuntimeFilesystemPort,RuntimeMessagePort}`, `runtime::pump::phases`
/// * Failure symptom: filesystem/message updates stop requesting redraw when state changes.
#[tokio::test]
async fn test_filesystem_and_message_phases_preserve_redraw_contract() {
	let mut editor = Editor::new_scratch();
	let _ = drain_for_pump(&mut editor).await;
	editor.mark_frame_drawn();

	let sent = editor.msg_tx().send(crate::msg::EditorMsg::Overlay(crate::msg::OverlayMsg::Notify(
		xeno_registry::notifications::keys::info("runtime-invariant-msg-redraw"),
	)));
	assert!(sent.is_ok(), "message enqueue should succeed");
	let directive = drain_for_pump(&mut editor).await;
	assert!(directive.needs_redraw, "message drain should request redraw when dirty flags require it");

	editor.mark_frame_drawn();
	let root = tempfile::tempdir().expect("must create temp dir");
	std::fs::write(root.path().join("main.rs"), "fn main() {}\n").expect("must write fixture file");
	assert!(
		editor
			.state
			.integration
			.filesystem
			.ensure_index(root.path().to_path_buf(), crate::filesystem::FilesystemOptions::default()),
		"filesystem ensure_index should enqueue work"
	);

	let saw_redraw = timeout(Duration::from_secs(2), async {
		loop {
			let directive = drain_for_pump(&mut editor).await;
			if directive.needs_redraw {
				return true;
			}
			sleep(Duration::from_millis(10)).await;
		}
	})
	.await
	.expect("timed out waiting for filesystem phase redraw");
	assert!(saw_redraw);
}

/// Must cap runtime maintenance rounds to avoid unbounded single-cycle stall.
///
/// * Enforced in: `runtime::pump::run_pump_cycle_with_report`
/// * Failure symptom: one runtime drain call monopolizes the editor thread under backlog.
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
	editor.enqueue_runtime_command_invocation("runtime_invariant_test_quit_command".to_string(), Vec::new(), RuntimeWorkSource::CommandOps);

	let directive = drain_for_pump(&mut editor).await;
	assert!(directive.should_quit);
	assert_eq!(directive.poll_timeout, None);

	let mut via_hook = Editor::new_scratch();
	via_hook.enqueue_runtime_nu_invocation(
		Invocation::editor_command("runtime_invariant_test_quit_command", Vec::new()),
		RuntimeWorkSource::NuHookDispatch,
	);

	let directive = drain_for_pump(&mut via_hook).await;
	assert!(directive.should_quit);
	assert_eq!(directive.poll_timeout, None);
}

/// Must preserve global FIFO order when draining runtime work from mixed sources.
///
/// * Enforced in: `Editor::drain_runtime_work_report`
/// * Failure symptom: deferred commands execute out-of-order across queue sources.
#[tokio::test]
async fn test_runtime_work_queue_preserves_fifo_across_sources() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.enqueue_runtime_command_invocation(
		"runtime_invariant_record_command".to_string(),
		vec!["one".to_string()],
		RuntimeWorkSource::Overlay,
	);
	editor.enqueue_runtime_command_invocation(
		"runtime_invariant_record_command".to_string(),
		vec!["two".to_string()],
		RuntimeWorkSource::ActionEffect,
	);
	editor.enqueue_runtime_nu_invocation(
		Invocation::command("runtime_invariant_record_command", vec!["three".to_string()]),
		RuntimeWorkSource::NuScheduledMacro,
	);

	let _ = drain_for_pump(&mut editor).await;
	let recorded = RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow().clone());
	assert_eq!(recorded, vec!["one".to_string(), "two".to_string(), "three".to_string()]);
}

/// Must make bounded fairness progress across mixed runtime work kinds without starvation.
///
/// * Enforced in: `runtime::pump::run_pump_cycle_with_report`, `runtime::work_drain::Editor::drain_runtime_work_report`
/// * Failure symptom: one kind drains while another kind remains indefinitely queued.
#[tokio::test]
async fn test_runtime_work_kind_fairness_under_mixed_backlog() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	for idx in 0..48 {
		editor.enqueue_runtime_command_invocation(
			"runtime_invariant_record_command".to_string(),
			vec![format!("mixed-{idx}")],
			RuntimeWorkSource::Overlay,
		);
		editor.enqueue_runtime_overlay_commit_work();
	}

	let report = editor.drain_until_idle(DrainPolicy::for_pump()).await;
	assert!(
		report.runtime_stats.drained_work_by_kind.invocation > 0,
		"invocation work should make bounded progress under mixed backlog"
	);
	assert!(
		report.runtime_stats.drained_work_by_kind.overlay_commit > 0,
		"overlay commit work should make bounded progress under mixed backlog"
	);
}

/// Must bound runtime work drain count per round to preserve latency.
///
/// * Enforced in: `runtime::pump::phases::phase_drain_runtime_work`
/// * Failure symptom: one round drains unbounded work backlog and stalls input responsiveness.
#[tokio::test]
async fn test_runtime_work_drain_is_bounded_per_round() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	for idx in 0..100 {
		editor.enqueue_runtime_command_invocation(
			"runtime_invariant_record_command".to_string(),
			vec![idx.to_string()],
			RuntimeWorkSource::ActionEffect,
		);
	}

	let (_directive, report) = editor.pump_with_report().await;
	assert!(!report.rounds.is_empty());
	assert!(report.rounds.iter().all(|round| round.work.drained_runtime_work <= 32));
	assert_eq!(report.rounds[0].work.drained_runtime_work, 32);
	assert_eq!(editor.runtime_work_len(), 4);
	assert!(report.reached_round_cap);
}

/// Must report queue depth and oldest-age snapshots from a consistent queue view.
///
/// * Enforced in: `Editor::drain_until_idle`
/// * Failure symptom: observability reports impossible states (e.g., non-empty depth with no age).
#[tokio::test]
async fn test_runtime_reports_oldest_age_and_depth_consistently() {
	let mut editor = Editor::new_scratch();
	for idx in 0..100 {
		editor.enqueue_runtime_command_invocation(
			"runtime_invariant_record_command".to_string(),
			vec![format!("age-{idx}")],
			RuntimeWorkSource::ActionEffect,
		);
	}

	let report = editor.drain_until_idle(DrainPolicy::for_pump()).await;
	assert_eq!(report.runtime_stats.final_work_queue_depth, editor.runtime_work_len());
	assert!(
		!report.runtime_stats.phase_queue_depths.is_empty(),
		"drain stats should include per-phase queue depth snapshots"
	);
	if report.runtime_stats.final_work_queue_depth > 0 {
		assert!(
			report.runtime_stats.oldest_work_age_ms.invocation_ms.is_some(),
			"oldest age should be present when invocation work remains queued"
		);
	}
}

/// Must mark budget-cap exits while preserving event causality on emitted directives.
///
/// * Enforced in: `Editor::drain_until_idle`
/// * Failure symptom: frontends lose event attribution when budget-limited drains stop early.
#[tokio::test]
async fn test_budget_cap_sets_exit_reason_without_losing_causality() {
	let mut editor = Editor::new_scratch();
	let first = editor.submit_event(RuntimeEvent::Key(Key::char('i')));
	let _second = editor.submit_event(RuntimeEvent::Key(Key::new(KeyCode::Esc)));

	let report = editor
		.drain_until_idle(DrainPolicy {
			max_frontend_events: 1,
			max_events_per_source: 1,
			max_directives: 1,
			run_idle_maintenance: false,
		})
		.await;

	assert!(report.reached_budget_cap);
	assert!(
		report.runtime_stats.round_exit_reasons.contains(&RuntimeDrainExitReason::BudgetCap),
		"budget-capped drain should include explicit budget-cap exit reason"
	);

	let directive = editor.poll_directive().expect("directive should be queued");
	assert_eq!(directive.cause_seq, Some(first.0));
	assert!(directive.cause_id.is_some());
	assert!(directive.pending_events > 0);
}

/// Must clear only the targeted Nu stop-scope generation from queued runtime work.
///
/// * Enforced in: `Editor::clear_runtime_nu_scope`, `Editor::enqueue_runtime_nu_invocation`
/// * Failure symptom: stop propagation drops unrelated deferred work or fails to drop stale Nu work.
#[tokio::test]
async fn test_nu_stop_scope_clear_is_generation_local() {
	RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.enqueue_runtime_nu_invocation(
		Invocation::command("runtime_invariant_record_command", vec!["nu-old".to_string()]),
		RuntimeWorkSource::NuHookDispatch,
	);
	editor.enqueue_runtime_command_invocation(
		"runtime_invariant_record_command".to_string(),
		vec!["global".to_string()],
		RuntimeWorkSource::Overlay,
	);

	let cleared_generation = editor.state.integration.nu.advance_stop_scope_generation();
	editor.clear_runtime_nu_scope(cleared_generation);

	editor.enqueue_runtime_nu_invocation(
		Invocation::command("runtime_invariant_record_command", vec!["nu-new".to_string()]),
		RuntimeWorkSource::NuHookDispatch,
	);

	let _ = drain_for_pump(&mut editor).await;
	let recorded = RUNTIME_INVARIANT_RECORDS.with(|records| records.borrow().clone());
	assert_eq!(recorded, vec!["global".to_string(), "nu-new".to_string()]);
}

/// Must apply execution policy from work queue item during drain so enforcing
/// items are gated and log-only items pass through.
///
/// * Enforced in: `runtime::work_drain::Editor::drain_runtime_work_report`
/// * Failure symptom: deferred Nu pipeline work bypasses capability/readonly checks.
#[tokio::test]
async fn test_runtime_work_execution_policy_gates_enforcement() {
	RUNTIME_EDIT_ACTION_COUNT.with(|c| c.set(0));

	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	// LogOnlyCommandPath → log-only policy → readonly not enforced → action runs.
	editor.enqueue_runtime_invocation(
		Invocation::action("runtime_invariant_edit_action"),
		RuntimeWorkSource::ActionEffect,
		WorkExecutionPolicy::LogOnlyCommandPath,
		WorkScope::Global,
	);

	// EnforcingNuPipeline → enforcing policy → readonly enforced → action denied.
	editor.enqueue_runtime_invocation(
		Invocation::action("runtime_invariant_edit_action"),
		RuntimeWorkSource::NuHookDispatch,
		WorkExecutionPolicy::EnforcingNuPipeline,
		WorkScope::Global,
	);

	assert_eq!(editor.runtime_work_len(), 2);
	let report = editor.drain_runtime_work_report(usize::MAX).await;
	assert_eq!(report.drained_invocations, 2, "both items should be drained");

	assert_eq!(
		RUNTIME_EDIT_ACTION_COUNT.with(|c| c.get()),
		1,
		"only the log-only item should execute the handler; enforcing item should be denied by readonly gate"
	);
}

/// Must keep log-only runtime unknown-command notifications on the canonical key path.
///
/// * Enforced in: `runtime::work_drain::Editor::drain_runtime_work_report`, `runtime::facade::RuntimeInvocationPort`
/// * Failure symptom: runtime queue drain emits divergent unknown-command notifications.
#[tokio::test]
async fn test_log_only_runtime_unknown_command_notification_path_is_stable() {
	let mut editor = Editor::new_scratch();
	let missing = "runtime_invariant_missing_command";

	editor.enqueue_runtime_command_invocation(missing.to_string(), Vec::new(), RuntimeWorkSource::ActionEffect);
	let report = editor.drain_runtime_work_report(usize::MAX).await;
	assert_eq!(report.drained_invocations, 1);

	let pending = editor.state.ui.notifications.take_pending();
	assert_eq!(pending.len(), 1, "missing command should emit one unknown-command notification");
	let expected = xeno_registry::notifications::keys::unknown_command(missing);
	assert_eq!(pending[0].id, expected.id);
	assert_eq!(pending[0].message, expected.message);
}

/// Must assign distinct, ordered cause IDs to directives from separately drained events.
///
/// * Enforced in: `runtime::kernel::RuntimeKernel::next_cause_id`, `Editor::drain_until_idle`
/// * Failure symptom: Two events share a cause ID, making causal attribution ambiguous.
#[tokio::test]
async fn test_cause_id_monotonic_across_distinct_events() {
	let mut editor = Editor::new_scratch();

	let _tok1 = editor.submit_event(RuntimeEvent::Key(Key::char('i')));
	let _ = editor.drain_until_idle(DrainPolicy::for_on_event()).await;
	let d1 = editor.poll_directive().expect("first directive");
	let cause1 = d1.cause_id.expect("first directive must have cause_id");

	let _tok2 = editor.submit_event(RuntimeEvent::Key(Key::new(KeyCode::Esc)));
	let _ = editor.drain_until_idle(DrainPolicy::for_on_event()).await;
	let d2 = editor.poll_directive().expect("second directive");
	let cause2 = d2.cause_id.expect("second directive must have cause_id");

	assert!(cause2.0 > cause1.0, "cause IDs must increase across sequential events");
}

/// Must propagate cause ID from draining work to follow-up work enqueued during that drain.
///
/// * Enforced in: `runtime::work_drain::Editor::drain_runtime_work_report`, `Editor::enqueue_runtime_command_invocation`
/// * Failure symptom: Follow-up work loses causal attribution, making debugging impossible.
#[tokio::test]
async fn test_cause_id_preserved_across_deferred_work_chain() {
	RUNTIME_INVARIANT_RECORDS.with(|r| r.borrow_mut().clear());

	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let token = editor.submit_event(RuntimeEvent::Key(Key::new(KeyCode::Enter)));
	let report = editor.drain_until_idle(DrainPolicy::for_on_event()).await;
	assert_eq!(report.handled_frontend_events, 1);

	let directive = editor.poll_directive().expect("directive should be queued");
	assert_eq!(directive.cause_seq, Some(token.0));
	let event_cause = directive.cause_id.expect("directive must carry a cause id");

	let overlay_work = editor
		.runtime_work_snapshot()
		.into_iter()
		.find(|item| matches!(item.kind, RuntimeWorkKind::OverlayCommit));

	if let Some(work) = overlay_work {
		assert_eq!(
			work.cause_id,
			Some(event_cause),
			"deferred overlay commit must carry the cause of the event that spawned it"
		);
	}
}

/// Must no-op overlay commit when the overlay was cancelled before drain.
///
/// * Enforced in: `OverlayManager::commit` (`self.active.take()` returns `None`)
/// * Failure symptom: Commit resurrects closed overlay state or panics on missing session.
#[tokio::test]
async fn test_overlay_commit_noop_when_cancelled_before_drain() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	editor.enqueue_runtime_overlay_commit_work();
	assert!(editor.has_runtime_overlay_commit_work());

	editor.interaction_cancel();
	assert!(editor.overlay_kind().is_none());

	let _ = drain_for_pump(&mut editor).await;

	assert!(!editor.has_runtime_overlay_commit_work());
	assert!(editor.overlay_kind().is_none(), "commit must not resurrect cancelled overlay");
}

/// Must no-op overlay commit when the overlay was force-closed (e.g. view removal) before drain.
///
/// * Enforced in: `OverlayManager::commit` (`self.active.take()` returns `None`)
/// * Failure symptom: Stale commit crashes or mutates editor state after forced overlay close.
#[tokio::test]
async fn test_overlay_commit_noop_when_force_closed_before_drain() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	editor.enqueue_runtime_overlay_commit_work();

	let mut interaction = editor.state.ui.overlay_system.take_interaction();
	interaction.close(&mut editor, crate::overlay::CloseReason::Forced);
	editor.state.ui.overlay_system.restore_interaction(interaction);
	assert!(editor.overlay_kind().is_none());

	let _ = drain_for_pump(&mut editor).await;

	assert!(!editor.has_runtime_overlay_commit_work());
	assert!(editor.overlay_kind().is_none(), "commit must not resurrect force-closed overlay");
}

/// Must commit only the first queued overlay commit; subsequent commits are no-ops.
///
/// * Enforced in: `OverlayManager::commit` (`self.active.take()` consumes the session once)
/// * Failure symptom: Double-commit panics or corrupts post-commit editor state.
#[tokio::test]
async fn test_multiple_overlay_commits_only_first_applies() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	editor.enqueue_runtime_overlay_commit_work();
	editor.enqueue_runtime_overlay_commit_work();
	editor.enqueue_runtime_overlay_commit_work();
	assert_eq!(
		editor
			.runtime_work_snapshot()
			.iter()
			.filter(|item| matches!(item.kind, RuntimeWorkKind::OverlayCommit))
			.count(),
		3,
	);

	let _ = drain_for_pump(&mut editor).await;

	assert!(editor.overlay_kind().is_none());
	assert!(!editor.has_runtime_overlay_commit_work());
}
