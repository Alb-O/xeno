use tracing::{trace, trace_span};
use xeno_invocation::CommandRoute;

use super::engine::InvocationEngine;
use crate::impls::Editor;
use crate::runtime::mailbox::{DeferredInvocationExecutionPolicy, DeferredInvocationScope, DeferredInvocationSource};
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy};

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
		self.enqueue_runtime_deferred_invocation(invocation, source, execution, scope);
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
		let _ = self.remove_runtime_deferred_invocation_scope(DeferredInvocationScope::NuStopScope(scope_generation));
	}
}
