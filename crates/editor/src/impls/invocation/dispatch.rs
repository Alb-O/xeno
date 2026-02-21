use tracing::{trace, trace_span};
use xeno_registry::actions::DeferredInvocationRequest;

use super::engine::InvocationEngine;
use crate::impls::Editor;
use crate::runtime::work_queue::{RuntimeWorkSource, WorkExecutionPolicy, WorkScope};
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy};

#[cfg(test)]
static RUN_INVOCATION_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn run_invocation_call_count() -> usize {
	RUN_INVOCATION_CALLS.load(std::sync::atomic::Ordering::SeqCst)
}

impl Editor {
	/// Test-only convenience: run an action through the canonical engine with enforcing defaults.
	#[cfg(test)]
	pub(crate) async fn invoke_action(&mut self, name: &str, count: usize, extend: bool, register: Option<char>, char_arg: Option<char>) -> InvocationOutcome {
		let invocation = if let Some(ch) = char_arg {
			Invocation::ActionWithChar {
				name: name.to_string(),
				count,
				extend,
				register,
				char_arg: ch,
			}
		} else {
			Invocation::Action {
				name: name.to_string(),
				count,
				extend,
				register,
			}
		};
		self.run_invocation(invocation, InvocationPolicy::enforcing()).await
	}

	/// Test-only convenience: run a command through the canonical engine with enforcing defaults.
	#[cfg(test)]
	pub(crate) async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationOutcome {
		self.run_invocation(Invocation::command(name.to_string(), args), InvocationPolicy::enforcing())
			.await
	}

	/// Executes an invocation through the canonical queue-driven engine.
	pub async fn run_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationOutcome {
		#[cfg(test)]
		RUN_INVOCATION_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		InvocationEngine::new(self, policy).run(invocation).await
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
			WorkScope::NuStopScope(self.state.integration.nu.current_stop_scope_generation()),
		);
	}

	/// Removes queued runtime work scoped to one Nu stop-propagation generation.
	pub(crate) fn clear_runtime_nu_scope(&mut self, scope_generation: u64) {
		let _ = self.clear_runtime_work_scope(WorkScope::NuStopScope(scope_generation));
	}

	/// Enqueues one typed invocation request from capability surfaces.
	///
	/// All deferred invocation requests use log-only command-path policy and global scope.
	/// Nu pipeline enforcement and scope binding happen at the Nu dispatch layer, not here.
	pub(crate) fn enqueue_runtime_invocation_request(&mut self, request: DeferredInvocationRequest, source: RuntimeWorkSource) {
		let invocation = match request {
			DeferredInvocationRequest::Command { name, args } => Invocation::command(name, args),
			DeferredInvocationRequest::EditorCommand { name, args } => Invocation::editor_command(name, args),
		};

		self.enqueue_runtime_invocation(invocation, source, WorkExecutionPolicy::LogOnlyCommandPath, WorkScope::Global);
	}
}
