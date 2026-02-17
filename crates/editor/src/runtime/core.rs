use std::time::Duration;

use xeno_primitives::{Key, Mode, MouseEvent};

use crate::Editor;
use crate::runtime::{DrainPolicy, DrainReport, ExternalEventKind, LoopDirectiveV2, RuntimeEventEnvelope, RuntimeEventSource, SubmitToken};

#[derive(Debug, Clone, Copy)]
pub struct LoopDirective {
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
	fn fallback_loop_directive(&self) -> LoopDirective {
		let needs_redraw = self.frame().needs_redraw;
		let poll_timeout = if matches!(self.mode(), Mode::Insert) || self.any_panel_open() || needs_redraw {
			Some(Duration::from_millis(16))
		} else {
			Some(Duration::from_millis(50))
		};

		LoopDirective {
			poll_timeout,
			needs_redraw,
			cursor_style: self.derive_cursor_style(),
			should_quit: false,
		}
	}

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

	fn from_v2_directive(directive: LoopDirectiveV2) -> LoopDirective {
		LoopDirective {
			poll_timeout: directive.poll_timeout,
			needs_redraw: directive.needs_redraw,
			cursor_style: directive.cursor_style,
			should_quit: directive.should_quit,
		}
	}

	async fn apply_frontend_event_envelope(&mut self, envelope: RuntimeEventEnvelope) {
		match envelope.event {
			RuntimeEvent::Key(key) => {
				let _ = self.handle_key(key).await;
			}
			RuntimeEvent::Mouse(mouse) => {
				let _ = self.handle_mouse(mouse).await;
			}
			RuntimeEvent::Paste(content) => {
				self.handle_paste(content);
			}
			RuntimeEvent::WindowResized { cols, rows } => {
				self.handle_window_resize(cols, rows);
			}
			RuntimeEvent::FocusIn => {
				self.handle_focus_in();
			}
			RuntimeEvent::FocusOut => {
				self.handle_focus_out();
			}
		}
	}

	fn apply_external_event(&mut self, kind: ExternalEventKind) {
		match kind {
			ExternalEventKind::QuitRequested => self.request_quit(),
			ExternalEventKind::Wake | ExternalEventKind::FilesystemChanged | ExternalEventKind::SchedulerCompleted | ExternalEventKind::RuntimeWorkQueued => {}
		}
	}

	async fn drain_until_idle_inner(&mut self, policy: DrainPolicy, publish_directives: bool) -> DrainReport {
		let mut report = DrainReport::default();

		if policy.max_directives == 0 {
			report.reached_budget_cap = true;
			return report;
		}

		let mut remaining_frontend = policy.max_frontend_events;
		let mut remaining_external = policy.max_external_events;
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

			if !consumed_event && remaining_external > 0 {
				if let Some(env) = self.state.runtime_kernel_mut().pop_external() {
					remaining_external = remaining_external.saturating_sub(1);
					report.handled_external_events = report.handled_external_events.saturating_add(1);
					cause_seq = Some(env.seq);
					consumed_event = true;
					self.apply_external_event(env.kind);
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

		if report.directives_emitted >= policy.max_directives
			&& (self.state.runtime_kernel().pending_event_count() > 0 || remaining_frontend > 0 || remaining_external > 0)
		{
			report.reached_budget_cap = true;
		}

		report
	}

	/// Submits one frontend runtime event into the runtime kernel queue.
	pub fn submit_event_with_source(&mut self, event: RuntimeEvent, source: RuntimeEventSource) -> SubmitToken {
		if let Some(rec) = &mut self.state.recorder {
			rec.record(&event);
		}
		SubmitToken(self.state.runtime_kernel_mut().enqueue_frontend(event, source))
	}

	/// Submits one frontend runtime event into the runtime kernel queue.
	pub fn submit_event(&mut self, event: RuntimeEvent) -> SubmitToken {
		self.submit_event_with_source(event, RuntimeEventSource::Frontend)
	}

	/// Submits one external runtime signal into the runtime kernel queue.
	pub fn submit_external_event(&mut self, kind: ExternalEventKind) -> SubmitToken {
		SubmitToken(self.state.runtime_kernel_mut().enqueue_external(kind))
	}

	/// Returns the next pending runtime loop directive.
	pub fn poll_directive(&mut self) -> Option<LoopDirectiveV2> {
		self.state.runtime_kernel_mut().pop_directive()
	}

	/// Drains queued runtime events and emits directives until policy limits are reached.
	pub async fn drain_until_idle(&mut self, policy: DrainPolicy) -> DrainReport {
		self.drain_until_idle_inner(policy, true).await
	}

	/// Runs one compatibility maintenance cycle.
	pub async fn pump(&mut self) -> LoopDirective {
		let report = self.drain_until_idle_inner(DrainPolicy::for_pump(), false).await;
		report
			.last_directive
			.map(Self::from_v2_directive)
			.unwrap_or_else(|| self.fallback_loop_directive())
	}

	/// Handle a single frontend event and then run compatibility maintenance.
	pub async fn on_event(&mut self, ev: RuntimeEvent) -> LoopDirective {
		let _ = self.submit_event(ev);
		let report = self.drain_until_idle_inner(DrainPolicy::for_on_event(), false).await;
		report
			.last_directive
			.map(Self::from_v2_directive)
			.unwrap_or_else(|| self.fallback_loop_directive())
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
