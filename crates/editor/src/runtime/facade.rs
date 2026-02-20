//! Runtime-facing subsystem facade traits and aggregate ports.
//!
//! This module narrows runtime orchestration to explicit mutation boundaries.
//! Pump phases consume [`RuntimePorts`] instead of reaching into `Editor` state
//! fields directly.

use xeno_primitives::Mode;

use crate::Editor;
use crate::runtime::work_drain::RuntimeWorkDrainReport;
use crate::scheduler::{DrainBudget, DrainStats};
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy};

/// Runtime filesystem mutation boundary.
pub(crate) trait RuntimeFilesystemPort {
	fn drain_filesystem_events(&mut self) -> usize;
	fn refresh_file_picker(&mut self);
	fn request_redraw(&mut self);
}

/// Runtime scheduler mutation boundary.
pub(crate) trait RuntimeSchedulerPort {
	fn scheduler_mode(&self) -> Mode;
	async fn drain_scheduler_budget(&mut self, budget: DrainBudget) -> DrainStats;
	fn record_scheduler_metrics(&self, drain_stats: &DrainStats);
	fn emit_scheduler_panic_notification(&mut self, drain_stats: &DrainStats);
}

/// Runtime overlay/runtime-work mutation boundary.
pub(crate) trait RuntimeOverlayPort {
	async fn drain_runtime_work(&mut self, max: usize) -> RuntimeWorkDrainReport;
	async fn apply_overlay_commit(&mut self);
}

/// Runtime invocation mutation boundary.
pub(crate) trait RuntimeInvocationPort {
	async fn run_runtime_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationOutcome;
	fn notify_unknown_command(&mut self, name: &str);
}

/// Runtime message-drain mutation boundary.
pub(crate) trait RuntimeMessagePort {
	fn drain_messages(&mut self) -> crate::impls::MessageDrainReport;
	fn request_redraw(&mut self);
}

/// Aggregate runtime ports object consumed by pump phases.
pub(crate) struct RuntimePorts<'a> {
	editor: &'a mut Editor,
}

impl<'a> RuntimePorts<'a> {
	pub(crate) fn new(editor: &'a mut Editor) -> Self {
		Self { editor }
	}

	pub(crate) fn editor(&self) -> &Editor {
		self.editor
	}

	pub(crate) fn editor_mut(&mut self) -> &mut Editor {
		self.editor
	}

	pub(crate) fn ui_tick_and_editor_tick(&mut self) {
		self.editor.ui_tick();
		self.editor.tick();
	}

	pub(crate) fn kick_nu_hook_eval(&mut self) {
		self.editor.kick_nu_hook_eval();
	}

	pub(crate) fn pending_event_count(&self) -> usize {
		self.editor.state.runtime_kernel().pending_event_count()
	}

	pub(crate) fn pending_runtime_work_count(&self) -> usize {
		self.editor.runtime_work_len()
	}
}

impl RuntimeFilesystemPort for RuntimePorts<'_> {
	fn drain_filesystem_events(&mut self) -> usize {
		self.editor.state.integration.filesystem.drain_events()
	}

	fn refresh_file_picker(&mut self) {
		self.editor.interaction_refresh_file_picker();
	}

	fn request_redraw(&mut self) {
		self.editor.frame_mut().needs_redraw = true;
	}
}

impl RuntimeSchedulerPort for RuntimePorts<'_> {
	fn scheduler_mode(&self) -> Mode {
		self.editor.mode()
	}

	async fn drain_scheduler_budget(&mut self, budget: DrainBudget) -> DrainStats {
		self.editor.work_scheduler_mut().drain_budget(budget).await
	}

	fn record_scheduler_metrics(&self, drain_stats: &DrainStats) {
		self.editor.metrics().record_hook_tick(drain_stats.completed, drain_stats.pending);
		self.editor
			.metrics()
			.record_worker_drain(drain_stats.completed, drain_stats.panicked, drain_stats.cancelled);
	}

	fn emit_scheduler_panic_notification(&mut self, drain_stats: &DrainStats) {
		if drain_stats.panicked == 0 {
			return;
		}

		use xeno_registry::notifications::{AutoDismiss, Level, Notification};
		let message = if let Some(sample) = &drain_stats.panic_sample {
			format!("worker tasks panicked: {} (first: {}) (see logs)", drain_stats.panicked, sample)
		} else {
			format!("worker tasks panicked: {} (see logs)", drain_stats.panicked)
		};
		self.editor
			.show_notification(Notification::new("xeno-editor::worker_task_panic", Level::Error, AutoDismiss::DEFAULT, message));
	}
}

impl RuntimeOverlayPort for RuntimePorts<'_> {
	async fn drain_runtime_work(&mut self, max: usize) -> RuntimeWorkDrainReport {
		self.editor.drain_runtime_work_report(max).await
	}

	async fn apply_overlay_commit(&mut self) {
		self.editor.interaction_commit().await;
	}
}

impl RuntimeInvocationPort for RuntimePorts<'_> {
	async fn run_runtime_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationOutcome {
		self.editor.run_invocation(invocation, policy).await
	}

	fn notify_unknown_command(&mut self, name: &str) {
		self.editor.show_notification(xeno_registry::notifications::keys::unknown_command(name));
	}
}

impl RuntimeMessagePort for RuntimePorts<'_> {
	fn drain_messages(&mut self) -> crate::impls::MessageDrainReport {
		self.editor.drain_messages_report()
	}

	fn request_redraw(&mut self) {
		self.editor.frame_mut().needs_redraw = true;
	}
}
