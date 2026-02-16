use tracing::warn;
use xeno_registry::actions::EditorContext;
use xeno_registry::{Capability, CapabilitySet, CommandError};

use crate::impls::Editor;
use crate::types::{InvocationPolicy, InvocationResult};

/// Canonical invocation target kind used for shared policy handling.
#[derive(Debug, Clone, Copy)]
pub(crate) enum InvocationKind {
	Action,
	Command,
	EditorCommand,
}

/// Encodes how required capabilities are represented by an invocation target.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CapabilityRequirement<'a> {
	Set(CapabilitySet),
	List(&'a [Capability]),
}

/// Shared preflight envelope used before executing invocation handlers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvocationSubject<'a> {
	pub(crate) kind: InvocationKind,
	pub(crate) name: &'a str,
	pub(crate) required_caps: CapabilityRequirement<'a>,
	pub(crate) mutates_buffer: bool,
}

impl<'a> InvocationSubject<'a> {
	pub(crate) fn action(name: &'a str, required_caps: CapabilitySet) -> Self {
		Self {
			kind: InvocationKind::Action,
			name,
			required_caps: CapabilityRequirement::Set(required_caps),
			mutates_buffer: requires_edit_capability_set(required_caps),
		}
	}

	pub(crate) fn command(name: &'a str, required_caps: CapabilitySet) -> Self {
		Self {
			kind: InvocationKind::Command,
			name,
			required_caps: CapabilityRequirement::Set(required_caps),
			mutates_buffer: requires_edit_capability_set(required_caps),
		}
	}

	pub(crate) fn editor_command(name: &'a str, required_caps: &'a [Capability]) -> Self {
		Self {
			kind: InvocationKind::EditorCommand,
			name,
			required_caps: CapabilityRequirement::List(required_caps),
			mutates_buffer: requires_edit_capability(required_caps),
		}
	}
}

/// Result of policy preflight for invocation execution.
#[derive(Debug)]
pub(crate) enum PreflightDecision {
	Proceed,
	Deny(InvocationResult),
}

impl Editor {
	/// Runs shared capability and readonly policy checks before invoking handlers.
	pub(crate) fn preflight_invocation_subject(&mut self, policy: InvocationPolicy, subject: InvocationSubject<'_>) -> PreflightDecision {
		if let Some(error) = capability_check_error(self, subject.required_caps)
			&& let Some(result) = handle_capability_violation(
				policy,
				error,
				|err| notify_capability_denied(self, subject.kind, err),
				|err| {
					warn!(
						kind = ?subject.kind,
						name = subject.name,
						error = %err,
						"Capability check failed (log-only mode)"
					);
				},
			) {
			return PreflightDecision::Deny(result);
		}

		if policy.enforce_readonly && subject.mutates_buffer && self.buffer().is_readonly() {
			return PreflightDecision::Deny(notify_readonly_denied(self));
		}

		PreflightDecision::Proceed
	}
}

fn capability_check_error(editor: &mut Editor, requirement: CapabilityRequirement<'_>) -> Option<CommandError> {
	match requirement {
		CapabilityRequirement::Set(required_caps) => {
			if required_caps.is_empty() {
				return None;
			}
			let mut caps = editor.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
			e_ctx.check_capability_set(required_caps).err()
		}
		CapabilityRequirement::List(required_caps) => {
			if required_caps.is_empty() {
				return None;
			}
			let mut caps = editor.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
			e_ctx.check_all_capabilities(required_caps).err()
		}
	}
}

fn notify_capability_denied(editor: &mut Editor, kind: InvocationKind, error: &CommandError) {
	match kind {
		InvocationKind::Action => editor.show_notification(xeno_registry::notifications::keys::action_error(error)),
		InvocationKind::Command | InvocationKind::EditorCommand => {
			let error = error.to_string();
			editor.show_notification(xeno_registry::notifications::keys::command_error(&error));
		}
	}
}

fn notify_readonly_denied(editor: &mut Editor) -> InvocationResult {
	editor.show_notification(xeno_registry::notifications::keys::BUFFER_READONLY.into());
	InvocationResult::ReadonlyDenied
}

fn requires_edit_capability_set(caps: CapabilitySet) -> bool {
	caps.contains(CapabilitySet::EDIT)
}

fn requires_edit_capability(caps: &[Capability]) -> bool {
	caps.iter().any(|cap| matches!(cap, Capability::Edit))
}

fn capability_error_result(error: &CommandError) -> InvocationResult {
	match error {
		CommandError::MissingCapability(cap) => InvocationResult::CapabilityDenied(*cap),
		_ => InvocationResult::CommandError(error.to_string()),
	}
}

pub(crate) fn handle_capability_violation(
	policy: InvocationPolicy,
	error: CommandError,
	on_enforce: impl FnOnce(&CommandError),
	on_log: impl FnOnce(&CommandError),
) -> Option<InvocationResult> {
	if policy.enforce_caps {
		on_enforce(&error);
		Some(capability_error_result(&error))
	} else {
		on_log(&error);
		None
	}
}
