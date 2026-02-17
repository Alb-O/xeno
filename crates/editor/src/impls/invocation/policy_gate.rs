use xeno_registry::actions::EditorContext;
use xeno_registry::{Capability, CapabilitySet, CommandError};

use crate::impls::Editor;
#[cfg(test)]
use crate::types::InvocationOutcome;
use crate::types::{InvocationPolicy, InvocationTarget};

/// Canonical invocation target kind used for shared policy handling.
#[derive(Debug, Clone, Copy)]
pub(crate) enum InvocationKind {
	Action,
	Command,
}

impl InvocationKind {
	pub(crate) const fn target(self) -> InvocationTarget {
		match self {
			Self::Action => InvocationTarget::Action,
			Self::Command => InvocationTarget::Command,
		}
	}
}

/// Encodes how required capabilities are represented by an invocation target.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RequiredCaps<'a> {
	Set(CapabilitySet),
	List(&'a [Capability]),
}

/// Shared policy gate envelope used before executing invocation handlers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvocationGateInput<'a> {
	pub(crate) kind: InvocationKind,
	pub(crate) name: &'a str,
	pub(crate) required_caps: RequiredCaps<'a>,
	pub(crate) mutates_buffer: bool,
}

impl<'a> InvocationGateInput<'a> {
	pub(crate) fn action(name: &'a str, required_caps: CapabilitySet) -> Self {
		Self {
			kind: InvocationKind::Action,
			name,
			required_caps: RequiredCaps::Set(required_caps),
			mutates_buffer: requires_edit_capability_set(required_caps),
		}
	}

	pub(crate) fn command(name: &'a str, required_caps: CapabilitySet) -> Self {
		Self {
			kind: InvocationKind::Command,
			name,
			required_caps: RequiredCaps::Set(required_caps),
			mutates_buffer: requires_edit_capability_set(required_caps),
		}
	}

	pub(crate) fn editor_command(name: &'a str, required_caps: &'a [Capability]) -> Self {
		Self {
			kind: InvocationKind::Command,
			name,
			required_caps: RequiredCaps::List(required_caps),
			mutates_buffer: requires_edit_capability(required_caps),
		}
	}
}

/// Result of policy gate checks for invocation execution.
#[derive(Debug)]
pub(crate) enum GateResult {
	Proceed,
	Deny(GateFailure),
}

/// Pure gate failure details emitted by policy checks.
#[derive(Debug)]
pub(crate) enum GateFailure {
	Capability(CommandError),
	Readonly,
}

impl Editor {
	/// Runs shared capability and readonly checks before invoking handlers.
	///
	/// Returns a pure gate decision plus optional log-only capability error
	/// metadata for caller-controlled logging.
	pub(crate) fn gate_invocation(&mut self, policy: InvocationPolicy, input: InvocationGateInput<'_>) -> (GateResult, Option<CommandError>) {
		if let Some(error) = capability_gate_error(self, input.required_caps) {
			match decide_capability_violation(policy, error) {
				CapabilityDecision::Deny(error) => {
					return (GateResult::Deny(GateFailure::Capability(error)), None);
				}
				CapabilityDecision::ProceedLogOnly(error) => {
					return (GateResult::Proceed, Some(error));
				}
			}
		}

		if policy.enforce_readonly && input.mutates_buffer && self.buffer().is_readonly() {
			return (GateResult::Deny(GateFailure::Readonly), None);
		}

		(GateResult::Proceed, None)
	}
}

fn capability_gate_error(editor: &mut Editor, requirement: RequiredCaps<'_>) -> Option<CommandError> {
	match requirement {
		RequiredCaps::Set(required_caps) => {
			if required_caps.is_empty() {
				return None;
			}
			let mut caps = editor.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
			e_ctx.check_capability_set(required_caps).err()
		}
		RequiredCaps::List(required_caps) => {
			if required_caps.is_empty() {
				return None;
			}
			let mut caps = editor.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
			e_ctx.check_all_capabilities(required_caps).err()
		}
	}
}

fn requires_edit_capability_set(caps: CapabilitySet) -> bool {
	caps.contains(CapabilitySet::EDIT)
}

fn requires_edit_capability(caps: &[Capability]) -> bool {
	caps.iter().any(|cap| matches!(cap, Capability::Edit))
}

#[cfg(test)]
fn capability_error_outcome(kind: InvocationKind, error: &CommandError) -> InvocationOutcome {
	match error {
		CommandError::MissingCapability(cap) => InvocationOutcome::capability_denied(kind.target(), *cap),
		_ => InvocationOutcome::command_error(kind.target(), error.to_string()),
	}
}

enum CapabilityDecision {
	Deny(CommandError),
	ProceedLogOnly(CommandError),
}

fn decide_capability_violation(policy: InvocationPolicy, error: CommandError) -> CapabilityDecision {
	if policy.enforce_caps {
		CapabilityDecision::Deny(error)
	} else {
		CapabilityDecision::ProceedLogOnly(error)
	}
}

#[cfg(test)]
pub(crate) fn handle_capability_violation(
	kind: InvocationKind,
	policy: InvocationPolicy,
	error: CommandError,
	on_enforce: impl FnOnce(&CommandError),
	on_log: impl FnOnce(&CommandError),
) -> Option<InvocationOutcome> {
	if policy.enforce_caps {
		on_enforce(&error);
		Some(capability_error_outcome(kind, &error))
	} else {
		on_log(&error);
		None
	}
}
