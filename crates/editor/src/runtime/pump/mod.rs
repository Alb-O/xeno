mod phases;
mod report;

use std::time::Duration;

pub(crate) use report::{MAX_PUMP_ROUNDS, PumpCycleReport, PumpPhase, RoundReport, RoundWorkFlags};
use xeno_primitives::Mode;

use super::core::LoopDirective;
use crate::Editor;

/// Runs one bounded-convergence maintenance cycle with a detailed report.
pub(crate) async fn run_pump_cycle_with_report(editor: &mut Editor) -> (LoopDirective, PumpCycleReport) {
	let mut report = PumpCycleReport::default();
	let mut should_quit = false;

	for round_idx in 0..MAX_PUMP_ROUNDS {
		let mut round = RoundReport {
			phases: Vec::new(),
			work: RoundWorkFlags::default(),
		};
		report.rounds_executed += 1;

		round.phases.push(PumpPhase::UiTickAndTick);
		phases::phase_ui_tick_and_editor_tick(editor);

		round.phases.push(PumpPhase::FilesystemEvents);
		let fs_outcome = phases::phase_filesystem_events(editor);
		round.work.filesystem_events = fs_outcome.drained_events;

		round.phases.push(PumpPhase::DrainMessages);
		let msg_outcome = phases::phase_drain_messages(editor);
		round.work.drained_messages = msg_outcome.drained_count;

		round.phases.push(PumpPhase::KickNuHookEval);
		phases::phase_kick_nu_hook_eval(editor);

		round.phases.push(PumpPhase::DrainScheduler);
		let scheduler_outcome = phases::phase_drain_scheduler(editor).await;
		round.work.scheduler_completions = scheduler_outcome.completed;

		round.phases.push(PumpPhase::DrainRuntimeWork);
		let runtime_work_outcome = phases::phase_drain_runtime_work(editor, phases::MAX_RUNTIME_WORK_ITEMS_PER_ROUND).await;
		round.work.drained_runtime_work = runtime_work_outcome.drained_count;
		if runtime_work_outcome.should_quit || editor.take_quit_request() {
			should_quit = true;
			report.should_quit = true;
			report.rounds.push(round);
			break;
		}

		let made_progress = round.work.made_progress();
		report.rounds.push(round);

		let last_round = round_idx + 1 == MAX_PUMP_ROUNDS;
		if made_progress && last_round {
			report.reached_round_cap = true;
		}
		if !made_progress || last_round {
			break;
		}
	}

	(finalize_loop_directive(editor, should_quit), report)
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
