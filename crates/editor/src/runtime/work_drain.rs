//! Drain policy for deferred runtime work queued in runtime work queue.

use crate::Editor;
use crate::runtime::work_queue::{RuntimeWorkKind, WorkExecutionPolicy};
use crate::types::{Invocation, InvocationStatus, PipelineDisposition, PipelineLogContext, classify_for_nu_pipeline, log_pipeline_non_ok};

/// Progress metadata for runtime-work drain work.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuntimeWorkDrainReport {
	pub(crate) drained_count: usize,
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

			match item.kind {
				RuntimeWorkKind::Invocation(queued) => {
					report.drained_invocations += 1;
					match queued.execution {
						WorkExecutionPolicy::EnforcingNuPipeline => {
							self.state.nu.inc_hook_depth();
							let result = self.run_invocation(queued.invocation, queued.execution.invocation_policy()).await;
							self.state.nu.dec_hook_depth();

							if matches!(classify_for_nu_pipeline(&result), PipelineDisposition::ShouldQuit) {
								report.should_quit = true;
								break;
							}
							log_pipeline_non_ok(&result, PipelineLogContext::HookDrain);
						}
						WorkExecutionPolicy::LogOnlyCommandPath => {
							let invocation = queued.invocation;
							let result = self.run_invocation(invocation.clone(), queued.execution.invocation_policy()).await;
							match result.status {
								InvocationStatus::NotFound => {
									if let Invocation::Command(command) = &invocation {
										self.show_notification(xeno_registry::notifications::keys::unknown_command(&command.name));
									}
								}
								InvocationStatus::Quit | InvocationStatus::ForceQuit => {
									report.should_quit = true;
									break;
								}
								_ => {}
							}
						}
					}
				}
				RuntimeWorkKind::OverlayCommit => {
					self.interaction_commit().await;
					report.applied_overlay_commits += 1;
				}
				#[cfg(feature = "lsp")]
				RuntimeWorkKind::WorkspaceEdit(edit) => {
					if let Err(err) = self.apply_workspace_edit(edit).await {
						self.notify(xeno_registry::notifications::keys::error(err.to_string()));
					}
					report.applied_workspace_edits += 1;
					self.frame_mut().needs_redraw = true;
				}
			}
		}

		report
	}
}
