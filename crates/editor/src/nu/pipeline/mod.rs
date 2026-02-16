//! Nu hook pipeline service.
//!
//! Owns queueing, async hook evaluation scheduling, stale-result protection,
//! and pending-invocation draining. This module centralizes hook lifecycle
//! transitions so `impls::invocation` can focus on action/command dispatch.

use tracing::{trace, warn};

use crate::impls::Editor;
#[cfg(test)]
use crate::nu::coordinator::runner::{NuExecKind, execute_with_restart};
use crate::nu::coordinator::{InFlightNuHook, QueuedNuHook};
use crate::types::{InvocationPolicy, InvocationResult};

/// Maximum pending Nu hooks before oldest are dropped.
const MAX_PENDING_NU_HOOKS: usize = 64;
/// Maximum Nu hooks drained per pump() cycle.
pub(crate) const MAX_NU_HOOKS_PER_PUMP: usize = 2;

/// Build hook args for action post hooks: `[name, result_label]`.
pub(crate) fn action_post_args(name: String, result: &InvocationResult) -> Vec<String> {
	vec![name, invocation_result_label(result).to_string()]
}

/// Build hook args for command/editor-command post hooks:
/// `[name, result_label, ...original_args]`.
pub(crate) fn command_post_args(name: String, result: &InvocationResult, args: Vec<String>) -> Vec<String> {
	let mut hook_args = vec![name, invocation_result_label(result).to_string()];
	hook_args.extend(args);
	hook_args
}

/// Build hook args for mode change: `[from_debug, to_debug]`.
pub(crate) fn mode_change_args(from: &xeno_primitives::Mode, to: &xeno_primitives::Mode) -> Vec<String> {
	vec![format!("{from:?}"), format!("{to:?}")]
}

/// Build hook args for buffer open: `[path, kind]`.
pub(crate) fn buffer_open_args(path: &std::path::Path, kind: &str) -> Vec<String> {
	vec![path.to_string_lossy().to_string(), kind.to_string()]
}

pub(crate) fn enqueue_nu_hook(editor: &mut Editor, hook: crate::nu::NuHook, args: Vec<String>) {
	// Don't enqueue during hook drain (prevents recursive hook chains).
	if editor.state.nu.in_hook_drain() {
		return;
	}

	// Skip if the hook function isn't defined.
	if !editor.state.nu.has_hook_decl(hook) {
		return;
	}

	if editor.state.nu.enqueue_hook(hook, args, MAX_PENDING_NU_HOOKS) {
		trace!(
			queue_len = editor.state.nu.hook_queue_len(),
			dropped_total = editor.state.nu.hook_dropped_total(),
			"nu_hook.drop_oldest"
		);
	}
}

pub(crate) fn enqueue_action_post_hook(editor: &mut Editor, name: String, result: &InvocationResult) {
	if !result.is_quit() {
		enqueue_nu_hook(editor, crate::nu::NuHook::ActionPost, action_post_args(name, result));
	}
}

pub(crate) fn enqueue_mode_change_hook(editor: &mut Editor, old: &xeno_primitives::Mode, new: &xeno_primitives::Mode) {
	enqueue_nu_hook(editor, crate::nu::NuHook::ModeChange, mode_change_args(old, new));
}

pub(crate) fn enqueue_buffer_open_hook(editor: &mut Editor, path: &std::path::Path, kind: &str) {
	enqueue_nu_hook(editor, crate::nu::NuHook::BufferOpen, buffer_open_args(path, kind));
}

/// Kicks one queued Nu hook evaluation onto the WorkScheduler.
///
/// Only kicks when no hook eval is already in flight (sequential evaluation
/// preserves the single-threaded NuExecutor contract). Every job uses an
/// epoch-scoped token for stale-result protection after runtime swaps.
pub(crate) fn kick_nu_hook_eval(editor: &mut Editor) {
	if editor.state.nu.hook_in_flight().is_some() || !editor.state.nu.has_queued_hooks() {
		return;
	}

	let Some(queued) = editor.state.nu.pop_queued_hook() else {
		return;
	};

	let fn_name = queued.hook.fn_name();
	let Some(decl_id) = editor.state.nu.hook_decl(queued.hook) else {
		return;
	};

	if editor.state.nu.ensure_executor().is_none() {
		return;
	}

	let limits = editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(crate::nu::DecodeLimits::hook_defaults, |c| c.hook_decode_limits());
	let nu_ctx = editor.build_nu_ctx("hook", fn_name, &queued.args);
	let env = vec![("XENO_CTX".to_string(), nu_ctx)];

	let executor_client = editor.state.nu.executor_client().expect("executor should exist");
	let msg_tx = editor.state.msg_tx.clone();

	let token = editor.state.nu.next_hook_eval_token();

	editor.state.nu.set_hook_in_flight(InFlightNuHook {
		token,
		hook: queued.hook,
		args: queued.args.clone(),
		retries: queued.retries,
	});

	let args_for_eval = queued.args;

	editor.state.work_scheduler.schedule(crate::scheduler::WorkItem {
		future: Box::pin(async move {
			let result = match executor_client.run(decl_id, args_for_eval, limits, env).await {
				Ok(invocations) => Ok(invocations),
				Err(crate::nu::executor::NuExecError::Eval(msg)) => Err(crate::msg::NuHookEvalError::Eval(msg)),
				Err(crate::nu::executor::NuExecError::Shutdown { .. }) => Err(crate::msg::NuHookEvalError::ExecutorShutdown),
				Err(crate::nu::executor::NuExecError::ReplyDropped) => Err(crate::msg::NuHookEvalError::ReplyDropped),
			};
			let _ = msg_tx.send(crate::msg::EditorMsg::NuHookEvalDone(crate::msg::NuHookEvalDoneMsg { token, result }));
		}),
		kind: crate::scheduler::WorkKind::NuHook,
		priority: xeno_registry::hooks::HookPriority::Interactive,
		doc_id: None,
	});
}

/// Applies the result of an async Nu hook evaluation.
///
/// Ignores stale results (token mismatch after runtime swap). On executor
/// death, restarts and retries once.
pub(crate) fn apply_nu_hook_eval_done(editor: &mut Editor, msg: crate::msg::NuHookEvalDoneMsg) -> crate::msg::Dirty {
	let in_flight_token = editor.state.nu.hook_in_flight_token();
	if in_flight_token != Some(msg.token) {
		// Stale result from a previous runtime — ignore.
		return crate::msg::Dirty::NONE;
	}

	let in_flight = editor.state.nu.take_hook_in_flight().expect("in-flight hook should exist");

	match msg.result {
		Ok(invocations) => {
			let dirty = if invocations.is_empty() {
				crate::msg::Dirty::NONE
			} else {
				crate::msg::Dirty::FULL
			};
			editor.state.nu.extend_pending_hook_invocations(invocations);
			dirty
		}
		Err(crate::msg::NuHookEvalError::Eval(error)) => {
			warn!(error = %error, "Nu hook evaluation failed");
			crate::msg::Dirty::NONE
		}
		Err(crate::msg::NuHookEvalError::ExecutorShutdown | crate::msg::NuHookEvalError::ReplyDropped) => {
			warn!(token = ?msg.token, "Nu executor died during hook eval, restarting");
			editor.state.nu.restart_executor();
			if in_flight.retries == 0 {
				editor.state.nu.push_front_queued_hook(QueuedNuHook {
					hook: in_flight.hook,
					args: in_flight.args,
					retries: 1,
				});
			} else {
				editor.state.nu.inc_hook_failed_total();
				warn!(failed_total = editor.state.nu.hook_failed_total(), "Nu hook retry exhausted");
			}
			crate::msg::Dirty::NONE
		}
	}
}

/// Drains pending Nu hook invocations under the depth guard.
///
/// Called from pump() after message drain. Executes invocations produced by
/// completed hook evaluations. Returns true if any produced quit.
pub(crate) async fn drain_nu_hook_invocations(editor: &mut Editor, max: usize) -> bool {
	if !editor.state.nu.has_pending_hook_invocations() {
		return false;
	}

	editor.state.nu.inc_hook_depth();

	for _ in 0..max {
		let Some(invocation) = editor.state.nu.pop_pending_hook_invocation() else {
			break;
		};

		let result = Box::pin(editor.run_invocation(invocation, InvocationPolicy::enforcing())).await;

		match result {
			InvocationResult::Ok => {}
			InvocationResult::Quit | InvocationResult::ForceQuit => {
				editor.state.nu.dec_hook_depth();
				return true;
			}
			InvocationResult::NotFound(target) => {
				warn!(target = %target, "Nu hook invocation not found");
			}
			InvocationResult::CapabilityDenied(cap) => {
				warn!(capability = ?cap, "Nu hook invocation denied by capability");
			}
			InvocationResult::ReadonlyDenied => {
				warn!("Nu hook invocation denied by readonly mode");
			}
			InvocationResult::CommandError(error) => {
				warn!(error = %error, "Nu hook invocation failed");
			}
		}
	}

	editor.state.nu.dec_hook_depth();
	false
}

/// Legacy synchronous drain for tests that need immediate hook evaluation.
///
/// Evaluates hooks synchronously via the executor (blocks on each one). Only
/// used in tests; production code uses kick + poll via pump().
#[cfg(test)]
pub(crate) async fn drain_nu_hook_queue(editor: &mut Editor, max: usize) -> bool {
	if !editor.state.nu.has_queued_hooks() {
		return false;
	}

	let to_drain = max.min(editor.state.nu.hook_queue_len());
	editor.state.nu.inc_hook_depth();

	for _ in 0..to_drain {
		let Some(queued) = editor.state.nu.pop_queued_hook() else {
			break;
		};

		match run_single_nu_hook_sync(editor, queued.hook, queued.args).await {
			Some(InvocationResult::Quit) | Some(InvocationResult::ForceQuit) => {
				editor.state.nu.dec_hook_depth();
				return true;
			}
			_ => {}
		}
	}

	editor.state.nu.dec_hook_depth();
	false
}

/// Synchronous single-hook evaluation for tests.
#[cfg(test)]
async fn run_single_nu_hook_sync(editor: &mut Editor, hook: crate::nu::NuHook, args: Vec<String>) -> Option<InvocationResult> {
	let fn_name = hook.fn_name();
	let decl_id = editor.state.nu.hook_decl(hook)?;
	editor.state.nu.ensure_executor()?;

	let limits = editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(crate::nu::DecodeLimits::hook_defaults, |c| c.hook_decode_limits());
	let nu_ctx = editor.build_nu_ctx("hook", fn_name, &args);

	let invocations = match execute_with_restart(&mut editor.state.nu, NuExecKind::Hook, fn_name, decl_id, args, limits, nu_ctx).await {
		Ok(invocations) => invocations,
		Err(error) => {
			warn!(hook = fn_name, error = ?error, "Nu hook failed");
			return None;
		}
	};

	for invocation in invocations {
		let result = Box::pin(editor.run_invocation(invocation, InvocationPolicy::enforcing())).await;

		match result {
			InvocationResult::Ok => {}
			InvocationResult::Quit | InvocationResult::ForceQuit => return Some(result),
			InvocationResult::NotFound(target) => {
				warn!(hook = fn_name, target = %target, "Nu hook invocation not found");
			}
			InvocationResult::CapabilityDenied(cap) => {
				warn!(hook = fn_name, capability = ?cap, "Nu hook invocation denied by capability");
			}
			InvocationResult::ReadonlyDenied => {
				warn!(hook = fn_name, "Nu hook invocation denied by readonly mode");
			}
			InvocationResult::CommandError(error) => {
				warn!(hook = fn_name, error = %error, "Nu hook invocation failed");
			}
		}
	}

	None
}

fn invocation_result_label(result: &InvocationResult) -> &'static str {
	match result {
		InvocationResult::Ok => "ok",
		InvocationResult::Quit => "quit",
		InvocationResult::ForceQuit => "force_quit",
		InvocationResult::NotFound(_) => "not_found",
		InvocationResult::CapabilityDenied(_) => "cap_denied",
		InvocationResult::ReadonlyDenied => "readonly",
		InvocationResult::CommandError(_) => "error",
	}
}

#[cfg(test)]
mod invariants;
