use crate::impls::Editor;
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

/// Shared policy gate envelope used before executing invocation handlers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvocationGateInput {
	pub(crate) kind: InvocationKind,
	pub(crate) mutates_buffer: bool,
}

impl InvocationGateInput {
	pub(crate) fn action(mutates_buffer: bool) -> Self {
		Self {
			kind: InvocationKind::Action,
			mutates_buffer,
		}
	}

	pub(crate) fn command(mutates_buffer: bool) -> Self {
		Self {
			kind: InvocationKind::Command,
			mutates_buffer,
		}
	}
}

/// Result of policy gate checks for invocation execution.
#[derive(Debug)]
pub(crate) enum GateResult {
	Proceed,
	DenyReadonly,
}

impl Editor {
	/// Checks whether the invocation should be blocked by readonly policy.
	///
	/// The only runtime gate is readonly enforcement: if the buffer is readonly
	/// and the item mutates the buffer, deny under enforcing policy.
	pub(crate) fn gate_invocation(&mut self, policy: InvocationPolicy, input: InvocationGateInput) -> GateResult {
		if policy.enforce_readonly && input.mutates_buffer && self.buffer().is_readonly() {
			return GateResult::DenyReadonly;
		}

		GateResult::Proceed
	}
}
