//! Drain policy for deferred runtime work queued in runtime work queue.

use crate::Editor;
use crate::runtime::work_queue::{RuntimeWorkKind, RuntimeWorkKindCounts, RuntimeWorkSource};
use crate::types::{Invocation, InvocationStatus, PipelineDisposition, PipelineLogContext, classify_for_nu_pipeline, log_pipeline_non_ok};

/// Progress metadata for runtime-work drain work.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuntimeWorkDrainReport {
	pub(crate) drained_count: usize,
	pub(crate) drained_by_kind: RuntimeWorkKindCounts,
	pub(crate) drained_invocations: usize,
	pub(crate) applied_overlay_commits: usize,
	#[cfg(feature = "lsp")]
	pub(crate) applied_workspace_edits: usize,
	pub(crate) should_quit: bool,
}

impl Editor {
	/// Drains runtime work queue items under a bounded per-round cap.
	pub(crate) async fn drain_runtime_work_report(&mut self, max: usize) -> RuntimeWorkDrainReport {
		let mut report = RuntimeWorkDrainReport::default();

		for _ in 0..max {
			let Some(item) = self.pop_runtime_work() else {
				break;
			};
			report.drained_count += 1;
			report.drained_by_kind.add_kind(item.kind_tag);
			let previous_cause = self.runtime_active_cause_id();
			self.set_runtime_active_cause_id(item.cause_id);
			tracing::trace!(
				runtime.work_seq = item.seq,
				runtime.cause_id = item.cause_id.map(|id| id.0),
				runtime.kind = ?item.kind_tag,
				"runtime.work.dequeue",
			);
			let mut should_break = false;

			match item.kind {
				RuntimeWorkKind::Invocation(queued) => {
					report.drained_invocations += 1;
					let invocation = queued.invocation;
					let source = queued.source;
					let is_nu_pipeline = matches!(source, RuntimeWorkSource::NuHookDispatch | RuntimeWorkSource::NuScheduledMacro);
					if is_nu_pipeline {
						self.state.nu.inc_hook_depth();
					}

					let policy = queued.execution.invocation_policy();
					tracing::trace!(
						runtime.work_seq = item.seq,
						runtime.cause_id = item.cause_id.map(|id| id.0),
						?source,
						?policy,
						scope = ?item.scope,
						invocation = %invocation.describe(),
						"invocation.runtime_work.run",
					);
					let result = self.run_invocation(invocation.clone(), policy).await;
					self.metrics().record_runtime_work_drained_total(item.kind_tag, Some(source));

					if is_nu_pipeline {
						self.state.nu.dec_hook_depth();
					}

					if is_nu_pipeline {
						if matches!(classify_for_nu_pipeline(&result), PipelineDisposition::ShouldQuit) {
							report.should_quit = true;
							should_break = true;
						}
						log_pipeline_non_ok(&result, PipelineLogContext::HookDrain);
						self.set_runtime_active_cause_id(previous_cause);
						if should_break {
							break;
						}
						continue;
					}

					match result.status {
						InvocationStatus::NotFound => {
							if let Invocation::Command(command) = &invocation {
								self.show_notification(xeno_registry::notifications::keys::unknown_command(&command.name));
							}
						}
						InvocationStatus::Quit | InvocationStatus::ForceQuit => {
							report.should_quit = true;
							should_break = true;
						}
						_ => {}
					}
				}
				RuntimeWorkKind::OverlayCommit => {
					self.interaction_commit().await;
					report.applied_overlay_commits += 1;
					self.metrics().record_runtime_work_drained_total(item.kind_tag, None);
				}
				#[cfg(feature = "lsp")]
				RuntimeWorkKind::WorkspaceEdit(edit) => {
					if let Err(err) = self.apply_workspace_edit(edit).await {
						self.notify(xeno_registry::notifications::keys::error(err.to_string()));
					}
					report.applied_workspace_edits += 1;
					self.frame_mut().needs_redraw = true;
					self.metrics().record_runtime_work_drained_total(item.kind_tag, None);
				}
			}

			self.set_runtime_active_cause_id(previous_cause);
			if should_break {
				break;
			}
		}

		report
	}
}
