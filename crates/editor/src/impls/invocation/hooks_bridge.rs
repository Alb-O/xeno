use crate::impls::Editor;
use crate::nu::ctx::NuCtxEvent;
use crate::types::InvocationOutcome;

/// Build a hook event for action post hooks.
pub(crate) fn action_post_event(name: String, result: &InvocationOutcome) -> NuCtxEvent {
	crate::nu::pipeline::action_post_event(name, result)
}

/// Build a hook event for command/editor-command post hooks.
pub(crate) fn command_post_event(name: String, result: &InvocationOutcome, args: Vec<String>) -> NuCtxEvent {
	crate::nu::pipeline::command_post_event(name, result, args)
}

/// Build an editor-command post hook event.
pub(crate) fn editor_command_post_event(name: String, result: &InvocationOutcome, args: Vec<String>) -> NuCtxEvent {
	crate::nu::pipeline::editor_command_post_event(name, result, args)
}

impl Editor {
	/// Enqueues a Nu post-hook for deferred evaluation during pump().
	///
	/// Coalesces consecutive identical hook types (keeps latest event) and
	/// drops the oldest entry when the queue exceeds pipeline capacity.
	pub(super) fn enqueue_nu_hook(&mut self, event: NuCtxEvent) {
		crate::nu::pipeline::enqueue_nu_hook(self, event);
	}

	/// Enqueues `on_hook` with action_post event directly for tests.
	#[cfg(test)]
	pub(crate) fn enqueue_action_post_hook(&mut self, name: String, result: &InvocationOutcome) {
		crate::nu::pipeline::enqueue_action_post_hook(self, name, result);
	}

	/// Enqueues `on_hook` with mode_change event after a mode transition.
	pub(crate) fn enqueue_mode_change_hook(&mut self, old: &xeno_primitives::Mode, new: &xeno_primitives::Mode) {
		crate::nu::pipeline::enqueue_mode_change_hook(self, old, new);
	}

	/// Enqueues `on_hook` with buffer_open event after a buffer is focused via navigation.
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

	/// Legacy synchronous drain for tests that need immediate hook evaluation.
	///
	/// Evaluates hooks synchronously via the executor (blocks on each one).
	/// Only used in tests; production code uses kick + poll via pump().
	#[cfg(test)]
	pub(crate) async fn drain_nu_hook_queue(&mut self, max: usize) -> bool {
		crate::nu::pipeline::drain_nu_hook_queue(self, max).await
	}
}
