use tracing::{trace, trace_span};
use xeno_invocation::CommandRoute;

use super::engine::InvocationEngine;
use crate::impls::Editor;
use crate::runtime::mailbox::{DeferredInvocationExecutionPolicy, DeferredInvocationScope, DeferredInvocationSource};
use crate::types::{
	Invocation, InvocationOutcome, InvocationPolicy, InvocationStatus, PipelineDisposition, PipelineLogContext, classify_for_nu_pipeline, log_pipeline_non_ok,
};

/// Progress metadata for runtime mailbox drain work.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeferredInvocationDrainReport {
	pub(crate) drained_count: usize,
	pub(crate) should_quit: bool,
}

impl Editor {
	/// Executes a named action with enforcement defaults.
	pub fn invoke_action(&mut self, name: &str, count: usize, extend: bool, register: Option<char>, char_arg: Option<char>) -> InvocationOutcome {
		self.run_action_invocation(name, count, extend, register, char_arg, InvocationPolicy::enforcing())
	}

	/// Executes a command invocation with enforcement defaults.
	pub async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationOutcome {
		self.run_command_invocation(name, &args, CommandRoute::Auto, InvocationPolicy::enforcing())
			.await
	}

	/// Executes an invocation through the canonical queue-driven engine.
	pub async fn run_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationOutcome {
		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		InvocationEngine::new(self, policy).run(invocation).await
	}

	/// Enqueues a deferred invocation into the runtime mailbox.
	pub(crate) fn enqueue_deferred_invocation(
		&mut self,
		invocation: Invocation,
		source: DeferredInvocationSource,
		execution: DeferredInvocationExecutionPolicy,
		scope: DeferredInvocationScope,
	) {
		self.state.invocation_mailbox.enqueue(invocation, source, execution, scope);
	}

	/// Enqueues a deferred command invocation into the runtime mailbox.
	pub(crate) fn enqueue_deferred_command(&mut self, name: String, args: Vec<String>, source: DeferredInvocationSource) {
		self.enqueue_deferred_invocation(
			Invocation::command(name, args),
			source,
			DeferredInvocationExecutionPolicy::LogOnlyCommandPath,
			DeferredInvocationScope::Global,
		);
	}

	/// Enqueues a deferred Nu-produced invocation into the runtime mailbox.
	pub(crate) fn enqueue_nu_deferred_invocation(&mut self, invocation: Invocation, source: DeferredInvocationSource) {
		self.enqueue_deferred_invocation(
			invocation,
			source,
			DeferredInvocationExecutionPolicy::EnforcingNuPipeline,
			DeferredInvocationScope::NuStopScope(self.state.nu.current_stop_scope_generation()),
		);
	}

	/// Removes deferred invocations scoped to a single Nu stop-propagation generation.
	pub(crate) fn clear_deferred_nu_scope(&mut self, scope_generation: u64) {
		let _ = self
			.state
			.invocation_mailbox
			.remove_scope(DeferredInvocationScope::NuStopScope(scope_generation));
	}

	/// Drains deferred invocations from the mailbox using item-attached execution policy.
	pub(crate) async fn drain_deferred_invocations_report(&mut self, max: usize) -> DeferredInvocationDrainReport {
		let mut report = DeferredInvocationDrainReport::default();

		for _ in 0..max {
			let Some(deferred) = self.state.invocation_mailbox.pop_front() else {
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
