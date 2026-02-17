use tracing::{trace, trace_span};
use xeno_invocation::CommandRoute;
use xeno_registry::actions::{DeferredInvocationKind, DeferredInvocationPolicy, DeferredInvocationRequest, DeferredInvocationScopeHint};

use super::engine::InvocationEngine;
use super::protocol::{InvocationCmd, InvocationEvt};
use crate::impls::Editor;
use crate::runtime::work_queue::{RuntimeWorkSource, WorkExecutionPolicy, WorkScope};
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

	/// Executes one typed invocation protocol command and returns a typed event.
	pub(crate) async fn run_invocation_cmd(&mut self, cmd: InvocationCmd) -> InvocationEvt {
		match cmd {
			InvocationCmd::Run {
				invocation,
				policy,
				source,
				scope,
				seq,
			} => {
				trace!(?source, ?scope, seq, invocation = %invocation.describe(), "invocation.protocol.run");
				InvocationEvt::Completed(self.run_invocation(invocation, policy).await)
			}
		}
	}

	/// Enqueues one runtime invocation item with explicit execution metadata.
	pub(crate) fn enqueue_runtime_invocation(&mut self, invocation: Invocation, source: RuntimeWorkSource, execution: WorkExecutionPolicy, scope: WorkScope) {
		self.enqueue_runtime_invocation_work(invocation, source, execution, scope);
	}

	/// Enqueues one runtime command invocation in global command-path scope.
	#[cfg(test)]
	pub(crate) fn enqueue_runtime_command_invocation(&mut self, name: String, args: Vec<String>, source: RuntimeWorkSource) {
		self.enqueue_runtime_invocation(
			Invocation::command(name, args),
			source,
			WorkExecutionPolicy::LogOnlyCommandPath,
			WorkScope::Global,
		);
	}

	/// Enqueues one runtime Nu-produced invocation in the current Nu scope.
	pub(crate) fn enqueue_runtime_nu_invocation(&mut self, invocation: Invocation, source: RuntimeWorkSource) {
		self.enqueue_runtime_invocation(
			invocation,
			source,
			WorkExecutionPolicy::EnforcingNuPipeline,
			WorkScope::NuStopScope(self.state.nu.current_stop_scope_generation()),
		);
	}

	/// Removes queued runtime work scoped to one Nu stop-propagation generation.
	pub(crate) fn clear_runtime_nu_scope(&mut self, scope_generation: u64) {
		let _ = self.clear_runtime_work_scope(WorkScope::NuStopScope(scope_generation));
	}

	/// Enqueues one typed invocation request from capability surfaces.
	pub(crate) fn enqueue_runtime_invocation_request(&mut self, request: DeferredInvocationRequest, source: RuntimeWorkSource) {
		let invocation = match request.kind {
			DeferredInvocationKind::Command { name, args } => Invocation::command(name, args),
			DeferredInvocationKind::EditorCommand { name, args } => Invocation::editor_command(name, args),
		};
		let execution = match request.policy {
			DeferredInvocationPolicy::LogOnlyCommandPath => WorkExecutionPolicy::LogOnlyCommandPath,
			DeferredInvocationPolicy::EnforcingNuPipeline => WorkExecutionPolicy::EnforcingNuPipeline,
		};
		let scope = match request.scope_hint {
			DeferredInvocationScopeHint::Global => WorkScope::Global,
			DeferredInvocationScopeHint::CurrentNuStopScope => WorkScope::NuStopScope(self.state.nu.current_stop_scope_generation()),
		};

		self.enqueue_runtime_invocation(invocation, source, execution, scope);
	}
}
