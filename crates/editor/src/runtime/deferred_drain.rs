//! Drain policy for deferred invocations queued in runtime-deferred state.

use crate::Editor;
use crate::runtime::mailbox::DeferredInvocationExecutionPolicy;
use crate::types::{Invocation, InvocationStatus, PipelineDisposition, PipelineLogContext, classify_for_nu_pipeline, log_pipeline_non_ok};

/// Progress metadata for deferred-invocation drain work.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeferredInvocationDrainReport {
	pub(crate) drained_count: usize,
	pub(crate) should_quit: bool,
}

impl Editor {
	/// Drains deferred invocations using item-attached execution policy.
	pub(crate) async fn drain_runtime_deferred_invocations_report(&mut self, max: usize) -> DeferredInvocationDrainReport {
		let mut report = DeferredInvocationDrainReport::default();

		for _ in 0..max {
			let Some(deferred) = self.pop_runtime_deferred_invocation() else {
				break;
			};
			report.drained_count += 1;

			match deferred.execution {
				DeferredInvocationExecutionPolicy::EnforcingNuPipeline => {
					self.state.nu.inc_hook_depth();
					let result = self.run_invocation(deferred.invocation, deferred.execution.invocation_policy()).await;
					self.state.nu.dec_hook_depth();

					if matches!(classify_for_nu_pipeline(&result), PipelineDisposition::ShouldQuit) {
						report.should_quit = true;
						break;
					}
					log_pipeline_non_ok(&result, PipelineLogContext::HookDrain);
				}
				DeferredInvocationExecutionPolicy::LogOnlyCommandPath => {
					let invocation = deferred.invocation;
					let result = self.run_invocation(invocation.clone(), deferred.execution.invocation_policy()).await;
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

		report
	}
}
