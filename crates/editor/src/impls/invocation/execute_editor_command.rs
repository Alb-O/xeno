use tracing::debug;

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::impls::invocation::preflight::{InvocationSubject, PreflightDecision};
use crate::types::{InvocationPolicy, InvocationResult};

impl Editor {
	/// Executes editor-direct commands with capability gating and policy checks.
	pub(crate) async fn run_editor_command_invocation(&mut self, name: &str, args: &[String], policy: InvocationPolicy) -> InvocationResult {
		let Some(editor_cmd) = find_editor_command(name) else {
			return InvocationResult::NotFound(format!("editor_command:{name}"));
		};

		debug!(command = name, "Executing editor command");

		let subject = InvocationSubject::editor_command(name, editor_cmd.required_caps);
		if let PreflightDecision::Deny(result) = self.preflight_invocation_subject(policy, subject) {
			return result;
		}

		let args_refs: Vec<&str> = args.iter().map(|arg| arg.as_str()).collect();
		let mut ctx = EditorCommandContext {
			editor: self,
			args: &args_refs,
			count: 1,
			register: None,
			user_data: editor_cmd.user_data,
		};

		let result = match (editor_cmd.handler)(&mut ctx).await {
			Ok(CommandOutcome::Ok) => InvocationResult::Ok,
			Ok(CommandOutcome::Quit) => InvocationResult::Quit,
			Ok(CommandOutcome::ForceQuit) => InvocationResult::ForceQuit,
			Err(error) => {
				let error = error.to_string();
				self.show_notification(xeno_registry::notifications::keys::command_error(&error));
				InvocationResult::CommandError(error)
			}
		};

		self.flush_effects();
		result
	}
}
