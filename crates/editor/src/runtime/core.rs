use std::time::Duration;

use xeno_primitives::{Key, Mode, MouseEvent};

use crate::Editor;
use crate::runtime::{DrainPolicy, DrainReport, LoopDirectiveV2, RuntimeEventEnvelope, SubmitToken};

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
	fn to_v2_directive(&self, directive: LoopDirective, cause_seq: Option<u64>, drained_runtime_work: usize) -> LoopDirectiveV2 {
		LoopDirectiveV2 {
			poll_timeout: directive.poll_timeout,
			needs_redraw: directive.needs_redraw,
			cursor_style: directive.cursor_style,
			should_quit: directive.should_quit,
			cause_seq,
			drained_runtime_work,
			pending_events: self.state.runtime_kernel().pending_event_count(),
		}
	}

	async fn apply_frontend_event_envelope(&mut self, envelope: RuntimeEventEnvelope) {
		let _ = self.apply_runtime_event_input(envelope.event).await;
	}

	async fn drain_until_idle_inner(&mut self, policy: DrainPolicy, publish_directives: bool) -> DrainReport {
		let mut report = DrainReport::default();

		if policy.max_directives == 0 {
			report.reached_budget_cap = true;
			return report;
		}

		let mut remaining_frontend = policy.max_frontend_events;
		let mut idle_maintenance_ran = false;

		for _ in 0..policy.max_directives {
			let mut cause_seq = None;
			let mut consumed_event = false;

			if remaining_frontend > 0 {
				if let Some(env) = self.state.runtime_kernel_mut().pop_frontend() {
					remaining_frontend = remaining_frontend.saturating_sub(1);
					report.handled_frontend_events = report.handled_frontend_events.saturating_add(1);
					cause_seq = Some(env.seq);
					consumed_event = true;
					self.apply_frontend_event_envelope(env).await;
				}
			}

			if !consumed_event {
				if !policy.run_idle_maintenance || idle_maintenance_ran {
					break;
				}
				idle_maintenance_ran = true;
			}

			let (directive, cycle_report) = super::pump::run_pump_cycle_with_report(self).await;
			let drained_runtime_work = cycle_report.rounds.iter().map(|round| round.work.drained_runtime_work).sum();
			let directive_v2 = self.to_v2_directive(directive, cause_seq, drained_runtime_work);

			if publish_directives {
				self.state.runtime_kernel_mut().push_directive(directive_v2);
			}

			report.directives_emitted = report.directives_emitted.saturating_add(1);
			report.last_directive = Some(directive_v2);

			if directive.should_quit {
				break;
			}
		}

		if report.directives_emitted >= policy.max_directives && self.state.runtime_kernel().pending_event_count() > 0 {
			report.reached_budget_cap = true;
		}

		report
	}

	/// Submits one frontend runtime event into the runtime kernel queue.
	pub fn submit_event(&mut self, event: RuntimeEvent) -> SubmitToken {
		if let Some(rec) = &mut self.state.recorder {
			rec.record(&event);
		}
		SubmitToken(self.state.runtime_kernel_mut().enqueue_frontend(event))
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
