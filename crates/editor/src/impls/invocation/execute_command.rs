use xeno_registry::RegistryEntry;
use xeno_registry::commands::{CommandContext, find_command};

use crate::commands::CommandOutcome;
use crate::impls::Editor;
use crate::impls::invocation::preflight::{InvocationSubject, PreflightDecision};
use crate::types::{InvocationPolicy, InvocationResult};

impl Editor {
	pub(crate) async fn run_command_invocation(&mut self, name: &str, args: &[String], policy: InvocationPolicy) -> InvocationResult {
		let Some(command_def) = find_command(name) else {
			// Don't notify - caller may want to try editor commands next
			return InvocationResult::NotFound(format!("command:{name}"));
		};

		let subject = InvocationSubject::command(name, command_def.required_caps());
		if let PreflightDecision::Deny(result) = self.preflight_invocation_subject(policy, subject) {
			return result;
		}

		let args_refs: Vec<&str> = args.iter().map(|arg| arg.as_str()).collect();
		let outcome = {
			let mut caps = self.caps();
			let mut ctx = CommandContext {
				editor: &mut caps,
				args: &args_refs,
				count: 1,
				register: None,
				user_data: command_def.user_data,
			};

			(command_def.handler)(&mut ctx).await
		};

		let result = match outcome {
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
