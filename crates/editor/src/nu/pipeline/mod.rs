//! Nu hook pipeline service.
//!
//! Owns queueing, async hook evaluation scheduling, stale-result protection,
//! pending-invocation draining, and hook-surface effect application.
//!
//! Hook completion transitions are delegated to `NuCoordinatorState`, while
//! effect semantics are delegated to `nu::effects`, keeping this module focused
//! on scheduling/orchestration.

use tracing::{trace, warn};

use crate::impls::Editor;
use crate::nu::coordinator::HookEvalFailureTransition;
#[cfg(test)]
use crate::nu::coordinator::runner::{NuExecKind, execute_with_restart};
use crate::nu::effects::{NuEffectApplyMode, apply_effect_batch};
use crate::nu::{NuCapability, NuDecodeSurface};
use crate::types::{InvocationOutcome, InvocationPolicy, InvocationStatus};

/// Maximum pending Nu hooks before oldest are dropped.
const MAX_PENDING_NU_HOOKS: usize = 64;
/// Maximum Nu hooks drained per pump() cycle.
pub(crate) const MAX_NU_HOOKS_PER_PUMP: usize = 2;

/// Report emitted after draining pending Nu hook-generated invocations.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NuHookInvocationDrainReport {
	pub(crate) drained_count: usize,
	pub(crate) should_quit: bool,
}

/// Build hook args for action post hooks: `[name, result_label]`.
pub(crate) fn action_post_args(name: String, result: &InvocationOutcome) -> Vec<String> {
	vec![name, result.label().to_string()]
}

/// Build hook args for command/editor-command post hooks:
/// `[name, result_label, ...original_args]`.
pub(crate) fn command_post_args(name: String, result: &InvocationOutcome, args: Vec<String>) -> Vec<String> {
	let mut hook_args = vec![name, result.label().to_string()];
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

#[cfg(test)]
pub(crate) fn enqueue_action_post_hook(editor: &mut Editor, name: String, result: &InvocationOutcome) {
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

	let budget = editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(crate::nu::DecodeBudget::hook_defaults, |c| c.hook_decode_budget());
	let nu_ctx = editor.build_nu_ctx("hook", fn_name, &queued.args);
	let env = vec![("XENO_CTX".to_string(), nu_ctx)];

	let executor_client = editor.state.nu.executor_client().expect("executor should exist");
	let msg_tx = editor.state.msg_tx.clone();

	let token = editor.state.nu.next_hook_eval_token();
	let args_for_eval = editor.state.nu.begin_hook_eval(token, queued);

	editor.state.work_scheduler.schedule(crate::scheduler::WorkItem {
		future: Box::pin(async move {
			let result = match executor_client.run(decl_id, NuDecodeSurface::Hook, args_for_eval, budget, env).await {
				Ok(effects) => Ok(effects),
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
	match msg.result {
		Ok(effects) => {
			if !editor.state.nu.complete_hook_eval(msg.token) {
				// Stale result from a previous runtime — ignore.
				return crate::msg::Dirty::NONE;
			}
			apply_hook_effect_batch(editor, effects)
		}
		Err(crate::msg::NuHookEvalError::Eval(error)) => {
			if !editor.state.nu.complete_hook_eval(msg.token) {
				return crate::msg::Dirty::NONE;
			}
			warn!(error = %error, "Nu hook evaluation failed");
			crate::msg::Dirty::NONE
		}
		Err(crate::msg::NuHookEvalError::ExecutorShutdown | crate::msg::NuHookEvalError::ReplyDropped) => {
			warn!(token = ?msg.token, "Nu executor died during hook eval, restarting");
			match editor.state.nu.complete_hook_eval_transport_failure(msg.token) {
				HookEvalFailureTransition::Stale => return crate::msg::Dirty::NONE,
				HookEvalFailureTransition::Retried => {}
				HookEvalFailureTransition::RetryExhausted { failed_total } => {
					warn!(failed_total, "Nu hook retry exhausted");
				}
			}
			editor.state.nu.restart_executor();
			crate::msg::Dirty::NONE
		}
	}
}

/// Drains pending Nu hook invocations and reports progress metadata.
///
/// Called from pump() after message drain. Executes invocations produced by
/// completed hook evaluations.
pub(crate) async fn drain_nu_hook_invocations_report(editor: &mut Editor, max: usize) -> NuHookInvocationDrainReport {
	if !editor.state.nu.has_pending_hook_invocations() {
		return NuHookInvocationDrainReport::default();
	}

	let mut report = NuHookInvocationDrainReport::default();
	editor.state.nu.inc_hook_depth();

	for _ in 0..max {
		let Some(invocation) = editor.state.nu.pop_pending_hook_invocation() else {
			break;
		};
		report.drained_count += 1;

		let result = editor.run_invocation(invocation, InvocationPolicy::enforcing()).await;

		match result.status {
			InvocationStatus::Ok => {}
			InvocationStatus::Quit | InvocationStatus::ForceQuit => {
				report.should_quit = true;
				break;
			}
			InvocationStatus::NotFound => {
				let target = result.detail.as_deref().unwrap_or("unknown");
				warn!(target = %target, "Nu hook invocation not found");
			}
			InvocationStatus::CapabilityDenied => {
				let cap = result.denied_capability;
				warn!(capability = ?cap, "Nu hook invocation denied by capability");
			}
			InvocationStatus::ReadonlyDenied => {
				warn!("Nu hook invocation denied by readonly mode");
			}
			InvocationStatus::CommandError => {
				let error = result.detail.as_deref().unwrap_or("unknown");
				warn!(error = %error, "Nu hook invocation failed");
			}
		}
	}

	editor.state.nu.dec_hook_depth();
	report
}

fn apply_hook_effect_batch(editor: &mut Editor, batch: crate::nu::NuEffectBatch) -> crate::msg::Dirty {
	let allowed = hook_allowed_capabilities(editor);
	let outcome = apply_effect_batch(editor, batch, NuEffectApplyMode::Hook, &allowed).expect("hook mode effect apply should not fail");

	if outcome.stop_requested {
		editor.state.nu.clear_hook_work_on_stop_propagation();
	} else if !outcome.dispatches.is_empty() {
		editor.state.nu.extend_pending_hook_invocations(outcome.dispatches);
	}

	outcome.dirty
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
			Some(InvocationOutcome {
				status: InvocationStatus::Quit | InvocationStatus::ForceQuit,
				..
			}) => {
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
async fn run_single_nu_hook_sync(editor: &mut Editor, hook: crate::nu::NuHook, args: Vec<String>) -> Option<InvocationOutcome> {
	let fn_name = hook.fn_name();
	let decl_id = editor.state.nu.hook_decl(hook)?;
	editor.state.nu.ensure_executor()?;

	let budget = editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(crate::nu::DecodeBudget::hook_defaults, |c| c.hook_decode_budget());
	let nu_ctx = editor.build_nu_ctx("hook", fn_name, &args);

	let effects = match execute_with_restart(
		&mut editor.state.nu,
		NuExecKind::Hook,
		fn_name,
		decl_id,
		args,
		NuDecodeSurface::Hook,
		budget,
		nu_ctx,
	)
	.await
	{
		Ok(effects) => effects,
		Err(error) => {
			warn!(hook = fn_name, error = ?error, "Nu hook failed");
			return None;
		}
	};

	let allowed = hook_allowed_capabilities(editor);
	let outcome = apply_effect_batch(editor, effects, NuEffectApplyMode::Hook, &allowed).expect("hook mode effect apply should not fail");
	if outcome.stop_requested {
		editor.state.nu.clear_hook_work_on_stop_propagation();
		return None;
	}

	for invocation in outcome.dispatches {
		let result = editor.run_invocation(invocation, InvocationPolicy::enforcing()).await;

		match result.status {
			InvocationStatus::Ok => {}
			InvocationStatus::Quit | InvocationStatus::ForceQuit => return Some(result),
			InvocationStatus::NotFound => {
				let target = result.detail.as_deref().unwrap_or("unknown");
				warn!(hook = fn_name, target = %target, "Nu hook invocation not found");
			}
			InvocationStatus::CapabilityDenied => {
				let cap = result.denied_capability;
				warn!(hook = fn_name, capability = ?cap, "Nu hook invocation denied by capability");
			}
			InvocationStatus::ReadonlyDenied => {
				warn!(hook = fn_name, "Nu hook invocation denied by readonly mode");
			}
			InvocationStatus::CommandError => {
				let error = result.detail.as_deref().unwrap_or("unknown");
				warn!(hook = fn_name, error = %error, "Nu hook invocation failed");
			}
		}
	}

	None
}

fn hook_allowed_capabilities(editor: &Editor) -> std::collections::HashSet<NuCapability> {
	editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(|| xeno_registry::config::NuConfig::default().hook_capabilities(), |cfg| cfg.hook_capabilities())
}

#[cfg(test)]
mod invariants;
