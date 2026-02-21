use xeno_registry::CommandError;
use xeno_registry::commands::CommandOutcome;
use xeno_registry::notifications::keys;

use super::policy_gate::{GateResult, InvocationGateInput};
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

	pub(super) fn deny_if_policy_blocks(&mut self, input: InvocationGateInput) -> Option<InvocationOutcome> {
		match self.editor.gate_invocation(self.policy, input) {
			GateResult::Proceed => None,
			GateResult::DenyReadonly => {
				self.editor.show_notification(xeno_registry::notifications::keys::BUFFER_READONLY.into());
				Some(InvocationOutcome::readonly_denied(input.kind.target()))
			}
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
