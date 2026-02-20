use std::cell::Cell;

use xeno_primitives::{Key, KeyCode};
use xeno_registry::actions::{DeferredInvocationPolicy, DeferredInvocationRequest, DeferredInvocationScopeHint};
use xeno_registry::{Capability, CommandError};

use super::policy_gate::{GateFailure, GateResult, InvocationGateInput, InvocationKind, RequiredCaps};
use super::{action_post_event, command_post_event, handle_capability_violation};
use crate::commands::{CommandError as EditorCommandError, CommandOutcome};
use crate::impls::Editor;
use crate::nu::ctx::NuCtxEvent;
use crate::runtime::work_queue::{RuntimeWorkKind, RuntimeWorkSource, WorkExecutionPolicy, WorkScope};
use crate::types::{
	Invocation, InvocationOutcome, InvocationPolicy, InvocationStatus, InvocationTarget, PipelineDisposition, classify_for_nu_pipeline,
	to_command_outcome_for_nu_run,
};

/// Must emit action post-hook event with name and result label.
///
/// * Enforced in: `action_post_event`, `Editor::run_invocation`
/// * Failure symptom: Nu `on_hook` receives malformed action_post event.
#[cfg_attr(test, test)]
pub(crate) fn test_action_post_event_shape() {
	let event = action_post_event("move_left".to_string(), &InvocationOutcome::ok(InvocationTarget::Action));
	assert!(matches!(event, NuCtxEvent::ActionPost { name, result } if name == "move_left" && result == "ok"));
}

/// Must emit command post-hook event with name, result label, and original args.
///
/// * Enforced in: `command_post_event`, `Editor::run_invocation`
/// * Failure symptom: Nu command post-hooks lose the original argument tail.
#[cfg_attr(test, test)]
pub(crate) fn test_command_post_event_shape() {
	let event = command_post_event(
		"write".to_string(),
		&InvocationOutcome::command_error(InvocationTarget::Command, "boom".to_string()),
		vec!["a".to_string(), "b".to_string()],
	);
	assert!(matches!(
		event,
		NuCtxEvent::CommandPost { name, result, args }
			if name == "write" && result == "error" && args == vec!["a".to_string(), "b".to_string()]
	));
}

/// Must return an invocation error when capability checks fail in enforcing mode.
///
/// * Enforced in: `handle_capability_violation`, `Editor::run_*_invocation`
/// * Failure symptom: missing capabilities execute mutating handlers anyway.
#[cfg_attr(test, test)]
pub(crate) fn test_capability_violation_enforcing_returns_error() {
	let result = handle_capability_violation(
		InvocationKind::Command,
		InvocationPolicy::enforcing(),
		CommandError::MissingCapability(Capability::Edit),
		|_| {},
		|_| panic!("log-only branch must not run in enforcing mode"),
	);
	assert!(matches!(
		&result,
		Some(InvocationOutcome {
			status: InvocationStatus::CapabilityDenied,
			..
		})
	));
	assert_eq!(result.and_then(|outcome| outcome.denied_capability()), Some(Capability::Edit));
}

/// Must continue execution in log-only mode while still reporting the violation.
///
/// * Enforced in: `handle_capability_violation`
/// * Failure symptom: migration mode either hard-fails unexpectedly or hides policy violations.
#[cfg_attr(test, test)]
pub(crate) fn test_capability_violation_log_only_continues() {
	thread_local! {
		static LOG_HIT: Cell<bool> = const { Cell::new(false) };
	}

	let result = handle_capability_violation(
		InvocationKind::Command,
		InvocationPolicy::default(),
		CommandError::MissingCapability(Capability::Edit),
		|_| panic!("enforcing branch must not run in log-only mode"),
		|_| LOG_HIT.with(|hit| hit.set(true)),
	);

	assert!(result.is_none());
	assert!(LOG_HIT.with(|hit| hit.get()), "log-only branch should be invoked");
}

/// Must deny mutating invocations on readonly buffers in enforcing mode.
///
/// * Enforced in: `Editor::gate_invocation`
/// * Failure symptom: command/action handlers mutate readonly buffers.
#[tokio::test]
pub(crate) async fn test_preflight_denies_readonly_mutating_subject() {
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let gate_input = InvocationGateInput {
		kind: InvocationKind::Command,
		name: "test",
		required_caps: RequiredCaps::Set(xeno_registry::CapabilitySet::empty()),
		mutates_buffer: true,
	};

	let decision = editor.gate_invocation(InvocationPolicy::enforcing(), gate_input);
	assert!(matches!(decision, (GateResult::Deny(GateFailure::Readonly), None)));
}

/// Must allow non-mutating invocations on readonly buffers.
///
/// * Enforced in: `Editor::gate_invocation`
/// * Failure symptom: readonly mode blocks harmless non-edit commands.
#[tokio::test]
pub(crate) async fn test_preflight_allows_non_mutating_subject_on_readonly_buffer() {
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let gate_input = InvocationGateInput {
		kind: InvocationKind::Command,
		name: "test",
		required_caps: RequiredCaps::Set(xeno_registry::CapabilitySet::empty()),
		mutates_buffer: false,
	};

	let decision = editor.gate_invocation(InvocationPolicy::enforcing(), gate_input);
	assert!(matches!(decision, (GateResult::Proceed, None)));
}

/// Must resolve auto-routed command invocations to editor commands before registry commands.
///
/// * Enforced in: `Editor::run_command_invocation_with_resolved_route`
/// * Failure symptom: editor commands shadowed by registry commands fail to execute.
#[tokio::test]
async fn test_auto_route_prefers_editor_commands() {
	let mut editor = Editor::new_scratch();
	let outcome = editor
		.run_invocation(Invocation::command("stats", Vec::<String>::new()), InvocationPolicy::enforcing())
		.await;
	assert!(matches!(outcome.status, InvocationStatus::Ok));
}

/// Must emit canonical not-found detail for unresolved auto-routed commands.
///
/// * Enforced in: `Editor::run_command_invocation_with_resolved_route`
/// * Failure symptom: callers cannot display consistent unknown-command diagnostics.
#[tokio::test]
async fn test_auto_route_not_found_reports_canonical_detail() {
	let mut editor = Editor::new_scratch();
	let outcome = editor
		.run_invocation(
			Invocation::command("definitely_missing_command", Vec::<String>::new()),
			InvocationPolicy::enforcing(),
		)
		.await;
	assert!(matches!(outcome.status, InvocationStatus::NotFound));
	assert_eq!(
		outcome.detail_text(),
		Some("command:definitely_missing_command"),
		"not-found detail should remain canonical"
	);
}

/// Must execute invocations through the canonical invocation engine with explicit policy.
///
/// * Enforced in: `Editor::run_invocation`
/// * Failure symptom: runtime work drain bypasses invocation policy/queue semantics.
#[tokio::test]
async fn test_run_invocation_enforcing_returns_ok_for_known_command() {
	let mut editor = Editor::new_scratch();
	let outcome = editor
		.run_invocation(Invocation::command("stats", Vec::<String>::new()), InvocationPolicy::enforcing())
		.await;
	assert!(matches!(outcome.status, InvocationStatus::Ok));
}

/// Must route keymap-produced invocations through `run_invocation`.
///
/// * Enforced in: `input::key_handling`, `Editor::run_invocation`
/// * Failure symptom: key dispatch bypasses invocation policy/hook/error boundaries.
#[tokio::test]
async fn test_keymap_dispatch_routes_through_run_invocation() {
	use super::dispatch::run_invocation_call_count;

	let mut editor = Editor::new_scratch();
	let before = run_invocation_call_count();
	let should_quit = editor.handle_key(Key::new(KeyCode::Char('l'))).await;
	assert!(!should_quit);
	assert!(
		run_invocation_call_count() > before,
		"keymap dispatch should increment run_invocation call counter"
	);
}

/// Must preserve source-aware deferred invocation policy/scope metadata when queueing runtime work.
///
/// * Enforced in: `Editor::enqueue_runtime_invocation_request`
/// * Failure symptom: runtime drain applies wrong policy/scope for deferred invocations.
#[cfg_attr(test, test)]
fn test_deferred_invocation_queue_preserves_source_policy_and_scope() {
	let mut editor = Editor::new_scratch();
	let current_nu_scope = editor.state.nu.current_stop_scope_generation();

	editor.enqueue_runtime_invocation_request(
		DeferredInvocationRequest::command("stats".to_string(), Vec::new()),
		RuntimeWorkSource::ActionEffect,
	);
	editor.enqueue_runtime_invocation_request(
		DeferredInvocationRequest::editor_command("stats".to_string(), Vec::new())
			.with_policy(DeferredInvocationPolicy::EnforcingNuPipeline)
			.with_scope_hint(DeferredInvocationScopeHint::CurrentNuStopScope),
		RuntimeWorkSource::NuHookDispatch,
	);

	let snapshot = editor.runtime_work_snapshot();
	assert_eq!(snapshot.len(), 2);

	let RuntimeWorkKind::Invocation(first) = &snapshot[0].kind else {
		panic!("first queued item should be invocation work");
	};
	assert_eq!(first.source, RuntimeWorkSource::ActionEffect);
	assert_eq!(first.execution, WorkExecutionPolicy::LogOnlyCommandPath);
	assert_eq!(snapshot[0].scope, WorkScope::Global);

	let RuntimeWorkKind::Invocation(second) = &snapshot[1].kind else {
		panic!("second queued item should be invocation work");
	};
	assert_eq!(second.source, RuntimeWorkSource::NuHookDispatch);
	assert_eq!(second.execution, WorkExecutionPolicy::EnforcingNuPipeline);
	assert_eq!(snapshot[1].scope, WorkScope::NuStopScope(current_nu_scope));
}

/// Must map Nu invocation outcomes into stable `nu-run` command results.
///
/// * Enforced in: `types::invocation::adapters::to_command_outcome_for_nu_run`
/// * Failure symptom: `nu-run` emits inconsistent command outcomes or errors.
#[cfg_attr(test, test)]
pub(crate) fn test_nu_run_outcome_bridge_mapping() {
	let ok = to_command_outcome_for_nu_run(&InvocationOutcome::ok(InvocationTarget::Nu), "nu:go");
	assert!(matches!(ok, Ok(CommandOutcome::Ok)));

	let not_found = to_command_outcome_for_nu_run(&InvocationOutcome::not_found(InvocationTarget::Nu, "nu:missing"), "nu:missing");
	assert!(matches!(not_found, Err(EditorCommandError::Failed(_))));
}

/// Must classify quit outcomes as terminal in Nu pipeline consumers.
///
/// * Enforced in: `types::invocation::adapters::classify_for_nu_pipeline`
/// * Failure symptom: hook pipeline keeps running after quit outcomes.
#[cfg_attr(test, test)]
pub(crate) fn test_pipeline_disposition_quit_classification() {
	let quit = InvocationOutcome::quit(InvocationTarget::Nu);
	assert_eq!(classify_for_nu_pipeline(&quit), PipelineDisposition::ShouldQuit);

	let ok = InvocationOutcome::ok(InvocationTarget::Nu);
	assert_eq!(classify_for_nu_pipeline(&ok), PipelineDisposition::Continue);
}
