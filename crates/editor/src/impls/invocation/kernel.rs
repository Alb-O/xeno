use tracing::warn;
use xeno_registry::CommandError;
use xeno_registry::commands::CommandOutcome;
use xeno_registry::notifications::keys;

use super::policy_gate::{GateFailure, GateResult, InvocationGateInput, InvocationKind};
use crate::impls::Editor;
use crate::types::{InvocationOutcome, InvocationPolicy, InvocationTarget};

pub(super) struct InvocationKernel<'a> {
	editor: &'a mut Editor,
	policy: InvocationPolicy,
}

impl<'a> InvocationKernel<'a> {
	pub(super) fn new(editor: &'a mut Editor, policy: InvocationPolicy) -> Self {
		Self { editor, policy }
	}

	pub(super) fn editor(&mut self) -> &mut Editor {
		self.editor
	}

	pub(super) fn deny_if_policy_blocks(&mut self, input: InvocationGateInput<'_>) -> Option<InvocationOutcome> {
		let (gate_result, log_only_error) = self.editor.gate_invocation(self.policy, input);
		if let Some(error) = log_only_error {
			warn!(
				kind = ?input.kind,
				name = input.name,
				error = %error,
				"Capability check failed (log-only mode)"
			);
		}

		match gate_result {
			GateResult::Proceed => None,
			GateResult::Deny(failure) => Some(self.map_gate_failure(input.kind, failure)),
		}
	}

	fn map_gate_failure(&mut self, kind: InvocationKind, failure: GateFailure) -> InvocationOutcome {
		match failure {
			GateFailure::Capability(error) => {
				self.notify_capability_denied(kind, &error);
				self.capability_error_outcome(kind, &error)
			}
			GateFailure::Readonly => {
				self.editor.show_notification(xeno_registry::notifications::keys::BUFFER_READONLY.into());
				InvocationOutcome::readonly_denied(kind.target())
			}
		}
	}

	fn notify_capability_denied(&mut self, kind: InvocationKind, error: &CommandError) {
		match kind {
			InvocationKind::Action => self.editor.show_notification(xeno_registry::notifications::keys::action_error(error)),
			InvocationKind::Command => {
				let error = error.to_string();
				self.editor.show_notification(xeno_registry::notifications::keys::command_error(&error));
			}
		}
	}

	fn capability_error_outcome(&self, kind: InvocationKind, error: &CommandError) -> InvocationOutcome {
		match error {
			CommandError::MissingCapability(cap) => InvocationOutcome::capability_denied(kind.target(), *cap),
			_ => InvocationOutcome::command_error(kind.target(), error.to_string()),
		}
	}

	pub(super) fn command_error(&self, target: InvocationTarget, detail: impl Into<String>) -> InvocationOutcome {
		InvocationOutcome::command_error(target, detail.into())
	}

	pub(super) fn command_error_with_notification(&mut self, target: InvocationTarget, detail: impl Into<String>) -> InvocationOutcome {
		let detail = detail.into();
		self.editor.show_notification(keys::command_error(&detail));
		self.command_error(target, detail)
	}

	fn map_command_outcome(&self, outcome: CommandOutcome, target: InvocationTarget) -> InvocationOutcome {
		match outcome {
			CommandOutcome::Ok => InvocationOutcome::ok(target),
			CommandOutcome::Quit => InvocationOutcome::quit(target),
			CommandOutcome::ForceQuit => InvocationOutcome::force_quit(target),
		}
	}

	pub(super) fn map_command_result(&mut self, target: InvocationTarget, result: Result<CommandOutcome, CommandError>) -> InvocationOutcome {
		match result {
			Ok(outcome) => self.map_command_outcome(outcome, target),
			Err(error) => self.command_error_with_notification(target, error.to_string()),
		}
	}

	pub(super) fn flush_effects_and_return(&mut self, outcome: InvocationOutcome) -> InvocationOutcome {
		self.editor.flush_effects();
		outcome
	}
}
