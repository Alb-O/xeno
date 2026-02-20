//! Nu hook pipeline service.
//!
//! Owns queueing, async hook evaluation scheduling, stale-result protection,
//! pending-invocation draining, and hook-surface effect application.
//!
//! All hooks are dispatched through a single `on_hook` Nu export. The hook
//! receives no positional arguments; all event data is injected via the
//! `$env.XENO_CTX.event` record. Hook type is determined by `event.type`.
//!
//! Hook completion transitions are delegated to `NuCoordinatorState`, while
//! effect semantics are delegated to `nu::effects`, keeping this module focused
//! on scheduling/orchestration.

use tracing::{trace, warn};

use crate::impls::Editor;
use crate::nu::ctx::NuCtxEvent;
use crate::nu::effects::{NuEffectApplyMode, apply_effect_batch};
use crate::nu::{NuCapability, NuDecodeSurface};
use crate::runtime::work_queue::RuntimeWorkSource;
use crate::types::InvocationOutcome;
#[cfg(test)]
use crate::types::{InvocationPolicy, PipelineDisposition, PipelineLogContext, classify_for_nu_pipeline, log_pipeline_non_ok};

/// Maximum pending Nu hooks before oldest are dropped.
const MAX_PENDING_NU_HOOKS: usize = 64;

/// Build a hook event for action post hooks.
pub(crate) fn action_post_event(name: String, result: &InvocationOutcome) -> NuCtxEvent {
	NuCtxEvent::ActionPost {
		name,
		result: result.label().to_string(),
	}
}

/// Build a hook event for command/editor-command post hooks.
pub(crate) fn command_post_event(name: String, result: &InvocationOutcome, args: Vec<String>) -> NuCtxEvent {
	NuCtxEvent::CommandPost {
		name,
		result: result.label().to_string(),
		args,
	}
}

/// Build an editor-command post hook event.
pub(crate) fn editor_command_post_event(name: String, result: &InvocationOutcome, args: Vec<String>) -> NuCtxEvent {
	NuCtxEvent::EditorCommandPost {
		name,
		result: result.label().to_string(),
		args,
	}
}

/// Build a hook event for mode change.
pub(crate) fn mode_change_event(from: &xeno_primitives::Mode, to: &xeno_primitives::Mode) -> NuCtxEvent {
	NuCtxEvent::ModeChange {
		from: format!("{from:?}"),
		to: format!("{to:?}"),
	}
}

/// Build a hook event for buffer open.
pub(crate) fn buffer_open_event(path: &std::path::Path, kind: &str) -> NuCtxEvent {
	NuCtxEvent::BufferOpen {
		path: path.to_string_lossy().to_string(),
		kind: kind.to_string(),
	}
}

pub(crate) fn enqueue_nu_hook(editor: &mut Editor, event: NuCtxEvent) {
	// Don't enqueue during hook drain (prevents recursive hook chains).
	if editor.state.integration.nu.in_hook_drain() {
		return;
	}

	// Skip if the hook function isn't defined.
	if !editor.state.integration.nu.has_on_hook_decl() {
		return;
	}

	if editor.state.integration.nu.enqueue_hook(event, MAX_PENDING_NU_HOOKS) {
		trace!(
			queue_len = editor.state.integration.nu.hook_queue_len(),
			dropped_total = editor.state.integration.nu.hook_dropped_total(),
			"nu_hook.drop_oldest"
		);
	}
}

#[cfg(test)]
pub(crate) fn enqueue_action_post_hook(editor: &mut Editor, name: String, result: &InvocationOutcome) {
	if !result.is_quit() {
		enqueue_nu_hook(editor, action_post_event(name, result));
	}
}

pub(crate) fn enqueue_mode_change_hook(editor: &mut Editor, old: &xeno_primitives::Mode, new: &xeno_primitives::Mode) {
	enqueue_nu_hook(editor, mode_change_event(old, new));
}

pub(crate) fn enqueue_buffer_open_hook(editor: &mut Editor, path: &std::path::Path, kind: &str) {
	enqueue_nu_hook(editor, buffer_open_event(path, kind));
}

/// Kicks one queued Nu hook evaluation onto the WorkScheduler.
///
/// Only kicks when no hook eval is already in flight (sequential evaluation
/// preserves the single-threaded NuExecutor contract). Every job uses an
/// epoch-scoped token for stale-result protection after runtime swaps.
pub(crate) fn kick_nu_hook_eval(editor: &mut Editor) {
	if editor.state.integration.nu.hook_in_flight().is_some() || !editor.state.integration.nu.has_queued_hooks() {
		return;
	}

	let Some(queued) = editor.state.integration.nu.pop_queued_hook() else {
		return;
	};

	let Some(decl_id) = editor.state.integration.nu.on_hook_decl() else {
		return;
	};

	if editor.state.integration.nu.ensure_executor().is_none() {
		return;
	}

	let budget = editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(crate::nu::DecodeBudget::hook_defaults, |c| c.hook_decode_budget());
	let nu_ctx = editor.build_nu_hook_ctx(&queued.event);
	let env = vec![("XENO_CTX".to_string(), nu_ctx)];
	let host = editor.build_nu_host_snapshot();

	let executor_client = editor.state.integration.nu.executor_client().expect("executor should exist");
	let msg_tx = editor.state.async_state.msg_tx.clone();

	let token = editor.state.integration.nu.next_hook_eval_token();
	let _event = editor.state.integration.nu.begin_hook_eval(token, queued);

	editor.state.integration.work_scheduler.schedule(crate::scheduler::WorkItem {
		future: Box::pin(async move {
			let result = executor_client
				.run(decl_id, NuDecodeSurface::Hook, vec![], budget, env, Some(Box::new(host)))
				.await;
			let _ = msg_tx.send(crate::msg::EditorMsg::NuHookEvalDone(crate::msg::NuHookEvalDoneMsg { token, result }));
		}),
		kind: crate::scheduler::WorkKind::NuHook,
		priority: xeno_registry::hooks::HookPriority::Interactive,
		doc_id: None,
	});
}

/// Applies the result of an async Nu hook evaluation.
///
/// Ignores stale results (token mismatch after runtime swap). Transport
/// errors are final for this hook call: executor retry happens only for
/// recoverable transport paths.
pub(crate) fn apply_nu_hook_eval_done(editor: &mut Editor, msg: crate::msg::NuHookEvalDoneMsg) -> crate::msg::Dirty {
	if !editor.state.integration.nu.complete_hook_eval(msg.token) {
		return crate::msg::Dirty::NONE;
	}

	match msg.result {
		Ok(effects) => apply_hook_effect_batch(editor, effects),
		Err(crate::nu::executor::NuExecError::Eval(error)) => {
			warn!(error = %error, "Nu hook evaluation failed");
			crate::msg::Dirty::NONE
		}
		Err(error) => {
			warn!(error = ?error, "Nu hook executor transport/closed error");
			crate::msg::Dirty::NONE
		}
	}
}

fn apply_hook_effect_batch(editor: &mut Editor, batch: crate::nu::NuEffectBatch) -> crate::msg::Dirty {
	let allowed = hook_allowed_capabilities(editor);
	let outcome = apply_effect_batch(editor, batch, NuEffectApplyMode::Hook, &allowed).expect("hook mode effect apply should not fail");

	if outcome.stop_requested {
		let scope_generation = editor.state.integration.nu.advance_stop_scope_generation();
		editor.state.integration.nu.clear_hook_work_on_stop_propagation();
		editor.clear_runtime_nu_scope(scope_generation);
	} else {
		for invocation in outcome.dispatches {
			editor.enqueue_runtime_nu_invocation(invocation, RuntimeWorkSource::NuHookDispatch);
		}
	}

	outcome.dirty
}

/// Legacy synchronous drain for tests that need immediate hook evaluation.
///
/// Evaluates hooks synchronously via the executor (blocks on each one). Only
/// used in tests; production code uses kick + runtime drain phases.
#[cfg(test)]
pub(crate) async fn drain_nu_hook_queue(editor: &mut Editor, max: usize) -> bool {
	if !editor.state.integration.nu.has_queued_hooks() {
		return false;
	}

	let to_drain = max.min(editor.state.integration.nu.hook_queue_len());
	editor.state.integration.nu.inc_hook_depth();

	for _ in 0..to_drain {
		let Some(queued) = editor.state.integration.nu.pop_queued_hook() else {
			break;
		};

		if let Some(result) = run_single_nu_hook_sync(editor, queued.event).await
			&& matches!(classify_for_nu_pipeline(&result), PipelineDisposition::ShouldQuit)
		{
			editor.state.integration.nu.dec_hook_depth();
			return true;
		}
	}

	editor.state.integration.nu.dec_hook_depth();
	false
}

/// Synchronous single-hook evaluation for tests.
#[cfg(test)]
async fn run_single_nu_hook_sync(editor: &mut Editor, event: NuCtxEvent) -> Option<InvocationOutcome> {
	let decl_id = editor.state.integration.nu.on_hook_decl()?;
	let executor = editor.state.integration.nu.ensure_executor()?;
	let executor_client = executor.client();

	let budget = editor
		.state
		.config
		.nu
		.as_ref()
		.map_or_else(crate::nu::DecodeBudget::hook_defaults, |c| c.hook_decode_budget());
	let nu_ctx = editor.build_nu_hook_ctx(&event);
	let env = vec![("XENO_CTX".to_string(), nu_ctx)];
	let host = editor.build_nu_host_snapshot();

	let effects = match executor_client
		.run(decl_id, NuDecodeSurface::Hook, vec![], budget, env, Some(Box::new(host)))
		.await
	{
		Ok(effects) => effects,
		Err(error) => {
			warn!(error = ?error, "Nu hook failed");
			return None;
		}
	};

	let allowed = hook_allowed_capabilities(editor);
	let outcome = apply_effect_batch(editor, effects, NuEffectApplyMode::Hook, &allowed).expect("hook mode effect apply should not fail");
	if outcome.stop_requested {
		editor.state.integration.nu.clear_hook_work_on_stop_propagation();
		return None;
	}

	for invocation in outcome.dispatches {
		let result = editor.run_invocation(invocation, InvocationPolicy::enforcing()).await;
		if matches!(classify_for_nu_pipeline(&result), PipelineDisposition::ShouldQuit) {
			return Some(result);
		}
		log_pipeline_non_ok(&result, PipelineLogContext::HookSync { hook: "on_hook" });
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
