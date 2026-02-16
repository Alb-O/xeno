use std::cell::Cell;

use xeno_registry::{Capability, CommandError};

use super::preflight::{CapabilityRequirement, InvocationKind, InvocationSubject, PreflightDecision};
use super::{action_post_args, command_post_args, handle_capability_violation};
use crate::impls::Editor;
use crate::types::{InvocationPolicy, InvocationResult};

/// Must emit action post-hook args as `[name, result_label]`.
///
/// * Enforced in: `action_post_args`, `Editor::run_invocation`
/// * Failure symptom: Nu `on_action_post` receives malformed arguments.
#[cfg_attr(test, test)]
pub(crate) fn test_action_post_args_shape() {
	let args = action_post_args("move_left".to_string(), &InvocationResult::Ok);
	assert_eq!(args, vec!["move_left".to_string(), "ok".to_string()]);
}

/// Must emit command/editor-command post-hook args as
/// `[name, result_label, ...original_args]`.
///
/// * Enforced in: `command_post_args`, `Editor::run_invocation`
/// * Failure symptom: Nu command post-hooks lose the original argument tail.
#[cfg_attr(test, test)]
pub(crate) fn test_command_post_args_prefix_and_tail() {
	let args = command_post_args(
		"write".to_string(),
		&InvocationResult::CommandError("boom".to_string()),
		vec!["a".to_string(), "b".to_string()],
	);
	assert_eq!(args, vec!["write".to_string(), "error".to_string(), "a".to_string(), "b".to_string()]);
}

/// Must return an invocation error when capability checks fail in enforcing mode.
///
/// * Enforced in: `handle_capability_violation`, `Editor::run_*_invocation`
/// * Failure symptom: missing capabilities execute mutating handlers anyway.
#[cfg_attr(test, test)]
pub(crate) fn test_capability_violation_enforcing_returns_error() {
	let result = handle_capability_violation(
		InvocationPolicy::enforcing(),
		CommandError::MissingCapability(Capability::Edit),
		|_| {},
		|_| panic!("log-only branch must not run in enforcing mode"),
	);
	assert!(matches!(result, Some(InvocationResult::CapabilityDenied(Capability::Edit))));
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
/// * Enforced in: `Editor::preflight_invocation_subject`
/// * Failure symptom: command/action handlers mutate readonly buffers.
#[cfg_attr(test, test)]
pub(crate) fn test_preflight_denies_readonly_mutating_subject() {
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let subject = InvocationSubject {
		kind: InvocationKind::Command,
		name: "test",
		required_caps: CapabilityRequirement::Set(xeno_registry::CapabilitySet::empty()),
		mutates_buffer: true,
	};

	let decision = editor.preflight_invocation_subject(InvocationPolicy::enforcing(), subject);
	assert!(matches!(decision, PreflightDecision::Deny(InvocationResult::ReadonlyDenied)));
}

/// Must allow non-mutating invocations on readonly buffers.
///
/// * Enforced in: `Editor::preflight_invocation_subject`
/// * Failure symptom: readonly mode blocks harmless non-edit commands.
#[cfg_attr(test, test)]
pub(crate) fn test_preflight_allows_non_mutating_subject_on_readonly_buffer() {
	let mut editor = Editor::new_scratch();
	editor.buffer_mut().set_readonly(true);

	let subject = InvocationSubject {
		kind: InvocationKind::Command,
		name: "test",
		required_caps: CapabilityRequirement::Set(xeno_registry::CapabilitySet::empty()),
		mutates_buffer: false,
	};

	let decision = editor.preflight_invocation_subject(InvocationPolicy::enforcing(), subject);
	assert!(matches!(decision, PreflightDecision::Proceed));
}
