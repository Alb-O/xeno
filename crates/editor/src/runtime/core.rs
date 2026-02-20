use std::time::Duration;

use xeno_primitives::{Key, Mode, MouseEvent};

use crate::Editor;
use crate::runtime::{
	DrainPolicy, DrainReport, LoopDirectiveV2, RuntimeCauseId, RuntimeDrainExitReason, RuntimeEventEnvelope, RuntimeEventSource, SubmitToken,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct LoopDirective {
	pub poll_timeout: Option<Duration>,
	pub needs_redraw: bool,
	pub cursor_style: CursorStyle,
	pub should_quit: bool,
}

/// Editor-defined cursor style (term maps to termina CSI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
	#[default]
	Block,
	Beam,
	Underline,
	Hidden,
}

/// Frontend-agnostic event stream consumed by the editor runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
	Key(Key),
	Mouse(MouseEvent),
	Paste(String),
	/// Viewport size expressed in text-grid cells.
	WindowResized {
		cols: u16,
		rows: u16,
	},
	FocusIn,
	FocusOut,
}

impl Editor {
	fn to_v2_directive(
		&self,
		directive: LoopDirective,
		cause_seq: Option<u64>,
		cause_id: Option<RuntimeCauseId>,
		drained_runtime_work: usize,
	) -> LoopDirectiveV2 {
		LoopDirectiveV2 {
			poll_timeout: directive.poll_timeout,
			needs_redraw: directive.needs_redraw,
			cursor_style: directive.cursor_style,
			should_quit: directive.should_quit,
			cause_seq,
			cause_id,
			drained_runtime_work,
			pending_events: self.state.runtime_kernel().pending_event_count(),
		}
	}

	pub(crate) fn runtime_active_cause_id(&self) -> Option<RuntimeCauseId> {
		self.state.runtime_active_cause_id()
	}

	pub(crate) fn set_runtime_active_cause_id(&mut self, cause_id: Option<RuntimeCauseId>) {
		self.state.set_runtime_active_cause_id(cause_id);
	}

	async fn apply_frontend_event_envelope(&mut self, envelope: RuntimeEventEnvelope) {
		tracing::trace!(
			runtime.event_seq = envelope.seq,
			runtime.cause_id = envelope.cause_id.0,
			?envelope.source,
			"runtime.event.apply",
		);
		let _ = self.apply_runtime_event_input(envelope.event).await;
	}

	async fn drain_until_idle_inner(&mut self, policy: DrainPolicy, publish_directives: bool) -> DrainReport {
		let mut report = DrainReport::default();
		let mut source_drained = [0usize; 3];
		let mut source_backpressure_blocked = false;

		if policy.max_directives == 0 {
			report.reached_budget_cap = true;
			report.runtime_stats.final_event_queue_depth = self.state.runtime_kernel().pending_event_count();
			report.runtime_stats.final_work_queue_depth = self.runtime_work_len();
			report.runtime_stats.oldest_work_age_ms = self.runtime_work_oldest_age_ms_by_kind();
			report.runtime_stats.round_exit_reasons.push(RuntimeDrainExitReason::BudgetCap);
			self.metrics().record_runtime_drain_exit_reason(RuntimeDrainExitReason::BudgetCap);
			return report;
		}

		let mut remaining_frontend = policy.max_frontend_events;
		let mut idle_maintenance_ran = false;

		for directive_idx in 0..policy.max_directives {
			let mut cause_seq = None;
			let mut cause_id = None;
			let mut consumed_event = false;
			let mut event_to_directive_latency_ms = None;

			if remaining_frontend > 0 {
				let maybe_front_source = self.state.runtime_kernel().peek_frontend().map(|env| env.source);
				if let Some(source) = maybe_front_source {
					let source_idx = source.idx();
					if source_drained[source_idx] < policy.max_events_per_source {
						if let Some(env) = self.state.runtime_kernel_mut().pop_frontend() {
							remaining_frontend = remaining_frontend.saturating_sub(1);
							source_drained[source_idx] = source_drained[source_idx].saturating_add(1);
							report.handled_frontend_events = report.handled_frontend_events.saturating_add(1);
							cause_seq = Some(env.seq);
							cause_id = Some(env.cause_id);
							event_to_directive_latency_ms = Some(env.submitted_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
							consumed_event = true;
							self.set_runtime_active_cause_id(cause_id);
							self.apply_frontend_event_envelope(env).await;
						}
					} else {
						source_backpressure_blocked = true;
					}
				}
			}

			if !consumed_event {
				self.set_runtime_active_cause_id(None);
				if !policy.run_idle_maintenance || idle_maintenance_ran {
					break;
				}
				idle_maintenance_ran = true;
			}

			let (directive, cycle_report) = super::pump::run_pump_cycle_with_report(self).await;
			self.set_runtime_active_cause_id(None);
			let drained_runtime_work = cycle_report.rounds.iter().map(|round| round.work.drained_runtime_work).sum();
			let directive_v2 = self.to_v2_directive(directive, cause_seq, cause_id, drained_runtime_work);

			if let Some(latency_ms) = event_to_directive_latency_ms {
				self.metrics().record_runtime_event_to_directive_latency_ms(latency_ms);
			}
			self.metrics().record_runtime_drain_rounds_executed(cycle_report.rounds_executed as u64);
			self.metrics().record_runtime_drain_exit_reason(cycle_report.exit_reason);

			report.runtime_stats.rounds_executed = report.runtime_stats.rounds_executed.saturating_add(cycle_report.rounds_executed);
			report.runtime_stats.phase_queue_depths.extend(cycle_report.phase_queue_depths.iter().copied());
			report.runtime_stats.drained_work_by_kind.add_from(cycle_report.drained_work_by_kind);
			report.runtime_stats.round_exit_reasons.push(cycle_report.exit_reason);

			if publish_directives {
				self.state.runtime_kernel_mut().push_directive(directive_v2);
			}

			report.directives_emitted = report.directives_emitted.saturating_add(1);
			report.last_directive = Some(directive_v2);
			tracing::trace!(
				runtime.round_idx = directive_idx,
				runtime.cause_id = cause_id.map(|id| id.0),
				runtime.event_seq = cause_seq,
				runtime.exit_reason = ?cycle_report.exit_reason,
				"runtime.directive.emitted",
			);

			if directive.should_quit {
				break;
			}
		}

		let pending_events = self.state.runtime_kernel().pending_event_count();
		if pending_events > 0 && (report.directives_emitted >= policy.max_directives || remaining_frontend == 0 || source_backpressure_blocked) {
			report.reached_budget_cap = true;
			report.runtime_stats.round_exit_reasons.push(RuntimeDrainExitReason::BudgetCap);
			self.metrics().record_runtime_drain_exit_reason(RuntimeDrainExitReason::BudgetCap);
		}

		report.runtime_stats.final_event_queue_depth = pending_events;
		report.runtime_stats.final_work_queue_depth = self.runtime_work_len();
		report.runtime_stats.oldest_work_age_ms = self.runtime_work_oldest_age_ms_by_kind();

		self.metrics().record_runtime_event_queue_depth(pending_events as u64);
		self.metrics()
			.record_runtime_work_queue_depth(report.runtime_stats.final_work_queue_depth as u64);
		self.metrics()
			.record_runtime_work_oldest_age_ms_by_kind(report.runtime_stats.oldest_work_age_ms);
		let pending_by_source = self.state.runtime_kernel().pending_event_count_by_source();
		tracing::trace!(
			pending_frontend = pending_by_source[RuntimeEventSource::Frontend.idx()],
			pending_replay = pending_by_source[RuntimeEventSource::Replay.idx()],
			pending_internal = pending_by_source[RuntimeEventSource::Internal.idx()],
			"runtime.event_queue.depth_by_source",
		);

		report
	}

	/// Submits one frontend runtime event into the runtime kernel queue.
	pub fn submit_event(&mut self, event: RuntimeEvent) -> SubmitToken {
		self.submit_event_from_source(event, RuntimeEventSource::Frontend)
	}

	/// Submits one runtime event with an explicit source tag.
	pub(crate) fn submit_event_from_source(&mut self, event: RuntimeEvent, source: RuntimeEventSource) -> SubmitToken {
		if matches!(source, RuntimeEventSource::Frontend)
			&& let Some(rec) = &mut self.state.recorder
		{
			rec.record(&event);
		}
		let (seq, cause_id) = self.state.runtime_kernel_mut().enqueue_frontend(event, source);
		let event_depth = self.state.runtime_kernel().pending_event_count() as u64;
		self.metrics().record_runtime_event_queue_depth(event_depth);
		tracing::trace!(
			runtime.event_seq = seq,
			runtime.cause_id = cause_id.0,
			?source,
			queue_depth = event_depth,
			"runtime.submit_event",
		);
		SubmitToken(seq)
	}

	/// Returns the next pending runtime loop directive.
	pub fn poll_directive(&mut self) -> Option<LoopDirectiveV2> {
		self.state.runtime_kernel_mut().pop_directive()
	}

	/// Drains queued runtime events and emits directives until policy limits are reached.
	pub async fn drain_until_idle(&mut self, policy: DrainPolicy) -> DrainReport {
		self.drain_until_idle_inner(policy, true).await
	}

	pub(crate) fn derive_cursor_style(&self) -> CursorStyle {
		self.ui().cursor_style().unwrap_or_else(|| match self.mode() {
			Mode::Insert => CursorStyle::Beam,
			_ => CursorStyle::Block,
		})
	}

	#[cfg(test)]
	pub(crate) async fn pump_with_report(&mut self) -> (LoopDirective, super::pump::PumpCycleReport) {
		super::pump::run_pump_cycle_with_report(self).await
	}
}
