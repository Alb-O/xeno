use crate::impls::Editor;
use crate::types::InvocationResult;

/// Maximum Nu hooks drained per pump() cycle.
pub(crate) const MAX_NU_HOOKS_PER_PUMP: usize = crate::nu::pipeline::MAX_NU_HOOKS_PER_PUMP;

/// Build hook args for action post hooks: `[name, result_label]`.
pub(crate) fn action_post_args(name: String, result: &InvocationResult) -> Vec<String> {
	crate::nu::pipeline::action_post_args(name, result)
}

/// Build hook args for command/editor-command post hooks: `[name, result_label, ...original_args]`.
pub(crate) fn command_post_args(name: String, result: &InvocationResult, args: Vec<String>) -> Vec<String> {
	crate::nu::pipeline::command_post_args(name, result, args)
}

impl Editor {
	/// Enqueues a Nu post-hook for deferred evaluation during pump().
	///
	/// Coalesces consecutive identical hook types (keeps latest args) and
	/// drops the oldest entry when the queue exceeds pipeline capacity.
	pub(super) fn enqueue_nu_hook(&mut self, hook: crate::nu::NuHook, args: Vec<String>) {
		crate::nu::pipeline::enqueue_nu_hook(self, hook, args);
	}

	/// Enqueues `on_action_post` hook directly for tests.
	#[cfg(test)]
	pub(crate) fn enqueue_action_post_hook(&mut self, name: String, result: &InvocationResult) {
		crate::nu::pipeline::enqueue_action_post_hook(self, name, result);
	}

	/// Enqueues `on_mode_change` hook after a mode transition.
	pub(crate) fn enqueue_mode_change_hook(&mut self, old: &xeno_primitives::Mode, new: &xeno_primitives::Mode) {
		crate::nu::pipeline::enqueue_mode_change_hook(self, old, new);
	}

	/// Enqueues `on_buffer_open` hook after a buffer is focused via navigation.
	pub(crate) fn enqueue_buffer_open_hook(&mut self, path: &std::path::Path, kind: &str) {
		crate::nu::pipeline::enqueue_buffer_open_hook(self, path, kind);
	}

	/// Kicks one queued Nu hook evaluation onto the WorkScheduler.
	///
	/// Only kicks when no hook eval is already in flight (sequential
	/// evaluation preserves the single-threaded NuExecutor contract).
	/// Uses runtime-generation tokens for stale-result protection after runtime
	/// swaps.
	pub(crate) fn kick_nu_hook_eval(&mut self) {
		crate::nu::pipeline::kick_nu_hook_eval(self);
	}

	/// Applies the result of an async Nu hook evaluation.
	///
	/// Ignores stale results (token mismatch after runtime swap).
	/// On executor death, restarts the executor and retries once.
	pub(crate) fn apply_nu_hook_eval_done(&mut self, msg: crate::msg::NuHookEvalDoneMsg) -> crate::msg::Dirty {
		crate::nu::pipeline::apply_nu_hook_eval_done(self, msg)
	}

	/// Drains pending Nu hook invocations and reports progress metadata.
	pub(crate) async fn drain_nu_hook_invocations_report(&mut self, max: usize) -> crate::nu::pipeline::NuHookInvocationDrainReport {
		crate::nu::pipeline::drain_nu_hook_invocations_report(self, max).await
	}

	/// Legacy synchronous drain for tests that need immediate hook evaluation.
	///
	/// Evaluates hooks synchronously via the executor (blocks on each one).
	/// Only used in tests; production code uses kick + poll via pump().
	#[cfg(test)]
	pub(crate) async fn drain_nu_hook_queue(&mut self, max: usize) -> bool {
		crate::nu::pipeline::drain_nu_hook_queue(self, max).await
	}
}
