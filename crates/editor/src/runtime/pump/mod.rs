mod phases;
mod report;

use std::time::Duration;

pub(crate) use report::{MAX_PUMP_ROUNDS, PumpCycleReport, PumpPhase, RoundReport, RoundWorkFlags};
use xeno_primitives::Mode;

use super::core::LoopDirective;
use crate::Editor;
use crate::runtime::protocol::{RuntimeDrainExitReason, RuntimePhaseQueueDepthSnapshot};

/// Runs one bounded-convergence maintenance cycle with a detailed report.
pub(crate) async fn run_pump_cycle_with_report(editor: &mut Editor) -> (LoopDirective, PumpCycleReport) {
	let mut report = PumpCycleReport::default();
	let mut should_quit = false;

	for round_idx in 0..MAX_PUMP_ROUNDS {
		let _round_span = tracing::trace_span!("runtime.round", runtime.round_idx = round_idx).entered();
		let mut round = RoundReport {
			phases: Vec::new(),
			phase_queue_depths: Vec::new(),
			work: RoundWorkFlags::default(),
			drained_work_by_kind: crate::runtime::work_queue::RuntimeWorkKindCounts::default(),
		};
		report.rounds_executed += 1;

		round.phases.push(PumpPhase::UiTickAndTick);
		phases::phase_ui_tick_and_editor_tick(editor);
		record_phase_snapshot(&mut report, &mut round, round_idx, PumpPhase::UiTickAndTick, editor);

		round.phases.push(PumpPhase::FilesystemEvents);
		let fs_outcome = phases::phase_filesystem_events(editor);
		round.work.filesystem_events = fs_outcome.drained_events;
		record_phase_snapshot(&mut report, &mut round, round_idx, PumpPhase::FilesystemEvents, editor);

		round.phases.push(PumpPhase::DrainMessages);
		let msg_outcome = phases::phase_drain_messages(editor);
		round.work.drained_messages = msg_outcome.drained_count;
		record_phase_snapshot(&mut report, &mut round, round_idx, PumpPhase::DrainMessages, editor);

		round.phases.push(PumpPhase::KickNuHookEval);
		phases::phase_kick_nu_hook_eval(editor);
		record_phase_snapshot(&mut report, &mut round, round_idx, PumpPhase::KickNuHookEval, editor);

		round.phases.push(PumpPhase::DrainScheduler);
		let scheduler_outcome = phases::phase_drain_scheduler(editor).await;
		round.work.scheduler_completions = scheduler_outcome.completed;
		record_phase_snapshot(&mut report, &mut round, round_idx, PumpPhase::DrainScheduler, editor);

		round.phases.push(PumpPhase::DrainRuntimeWork);
		let runtime_work_outcome = phases::phase_drain_runtime_work(editor, phases::MAX_RUNTIME_WORK_ITEMS_PER_ROUND).await;
		round.work.drained_runtime_work = runtime_work_outcome.drained_count;
		round.drained_work_by_kind = runtime_work_outcome.drained_by_kind;
		report.drained_work_by_kind.add_from(runtime_work_outcome.drained_by_kind);
		record_phase_snapshot(&mut report, &mut round, round_idx, PumpPhase::DrainRuntimeWork, editor);
		if runtime_work_outcome.should_quit || editor.take_quit_request() {
			should_quit = true;
			report.should_quit = true;
			report.exit_reason = RuntimeDrainExitReason::Quit;
			report.rounds.push(round);
			break;
		}

		let made_progress = round.work.made_progress();
		report.rounds.push(round);

		let last_round = round_idx + 1 == MAX_PUMP_ROUNDS;
		if made_progress && last_round {
			report.reached_round_cap = true;
			report.exit_reason = RuntimeDrainExitReason::RoundCap;
		}
		if !made_progress || last_round {
			if !last_round && !made_progress {
				report.exit_reason = RuntimeDrainExitReason::Idle;
			}
			break;
		}
	}

	(finalize_loop_directive(editor, should_quit), report)
}

fn record_phase_snapshot(report: &mut PumpCycleReport, round: &mut RoundReport, round_idx: usize, phase: PumpPhase, editor: &Editor) {
	let snapshot = RuntimePhaseQueueDepthSnapshot {
		round_idx,
		phase: phase.label(),
		event_queue_depth: editor.state.runtime_kernel().pending_event_count(),
		work_queue_depth: editor.runtime_work_len(),
	};
	round.phase_queue_depths.push(snapshot);
	report.phase_queue_depths.push(snapshot);
	tracing::trace!(
		runtime.round_idx = round_idx,
		runtime.phase = snapshot.phase,
		event_queue_depth = snapshot.event_queue_depth,
		work_queue_depth = snapshot.work_queue_depth,
		"runtime.phase.snapshot",
	);
}

fn finalize_loop_directive(editor: &Editor, should_quit: bool) -> LoopDirective {
	if should_quit {
		return LoopDirective {
			poll_timeout: None,
			needs_redraw: true,
			cursor_style: editor.derive_cursor_style(),
			should_quit: true,
		};
	}

	let needs_redraw = editor.frame().needs_redraw;
	let poll_timeout = if matches!(editor.mode(), Mode::Insert) || editor.any_panel_open() || needs_redraw {
		Some(Duration::from_millis(16))
	} else {
		Some(Duration::from_millis(50))
	};

	LoopDirective {
		poll_timeout,
		needs_redraw,
		cursor_style: editor.derive_cursor_style(),
		should_quit: false,
	}
}
