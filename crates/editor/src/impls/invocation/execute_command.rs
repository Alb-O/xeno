use xeno_invocation::CommandRoute;
use xeno_registry::RegistryEntry;
use xeno_registry::commands::{CommandContext, find_command};

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::impls::invocation::preflight::{InvocationSubject, PreflightDecision};
use crate::types::{InvocationOutcome, InvocationPolicy, InvocationTarget};

enum ResolvedCommandTarget {
	Editor(&'static crate::commands::EditorCommandDef),
	Registry(xeno_registry::commands::CommandRef),
	Missing,
}

struct CommandResolution {
	resolved_route: CommandRoute,
	target: ResolvedCommandTarget,
}

fn resolve_command_target(name: &str, route: CommandRoute) -> CommandResolution {
	match route {
		CommandRoute::Editor => CommandResolution {
			resolved_route: CommandRoute::Editor,
			target: find_editor_command(name)
				.map(ResolvedCommandTarget::Editor)
				.unwrap_or(ResolvedCommandTarget::Missing),
		},
		CommandRoute::Registry => CommandResolution {
			resolved_route: CommandRoute::Registry,
			target: find_command(name)
				.map(ResolvedCommandTarget::Registry)
				.unwrap_or(ResolvedCommandTarget::Missing),
		},
		CommandRoute::Auto => {
			if let Some(editor_cmd) = find_editor_command(name) {
				return CommandResolution {
					resolved_route: CommandRoute::Editor,
					target: ResolvedCommandTarget::Editor(editor_cmd),
				};
			}
			if let Some(command_def) = find_command(name) {
				return CommandResolution {
					resolved_route: CommandRoute::Registry,
					target: ResolvedCommandTarget::Registry(command_def),
				};
			}
			CommandResolution {
				resolved_route: CommandRoute::Auto,
				target: ResolvedCommandTarget::Missing,
			}
		}
	}
}

impl Editor {
	pub(crate) async fn run_command_invocation(&mut self, name: &str, args: &[String], route: CommandRoute, policy: InvocationPolicy) -> InvocationOutcome {
		self.run_command_invocation_with_resolved_route(name, args, route, policy).await.0
	}

	pub(crate) async fn run_command_invocation_with_resolved_route(
		&mut self,
		name: &str,
		args: &[String],
		route: CommandRoute,
		policy: InvocationPolicy,
	) -> (InvocationOutcome, CommandRoute) {
		let CommandResolution { resolved_route, target } = resolve_command_target(name, route);
		let outcome = match target {
			ResolvedCommandTarget::Editor(editor_cmd) => self.execute_editor_command(name, args, editor_cmd, policy).await,
			ResolvedCommandTarget::Registry(command_def) => self.execute_registry_command(name, args, command_def, policy).await,
			ResolvedCommandTarget::Missing => InvocationOutcome::not_found(InvocationTarget::Command, format!("command:{name}")),
		};
		(outcome, resolved_route)
	}

	async fn execute_registry_command(
		&mut self,
		name: &str,
		args: &[String],
		command_def: xeno_registry::commands::CommandRef,
		policy: InvocationPolicy,
	) -> InvocationOutcome {
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
			Ok(CommandOutcome::Ok) => InvocationOutcome::ok(InvocationTarget::Command),
			Ok(CommandOutcome::Quit) => InvocationOutcome::quit(InvocationTarget::Command),
			Ok(CommandOutcome::ForceQuit) => InvocationOutcome::force_quit(InvocationTarget::Command),
			Err(error) => {
				let error = error.to_string();
				self.show_notification(xeno_registry::notifications::keys::command_error(&error));
				InvocationOutcome::command_error(InvocationTarget::Command, error)
			}
		};

		self.flush_effects();
		result
	}

	async fn execute_editor_command(
		&mut self,
		name: &str,
		args: &[String],
		editor_cmd: &'static crate::commands::EditorCommandDef,
		policy: InvocationPolicy,
	) -> InvocationOutcome {
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
			Ok(CommandOutcome::Ok) => InvocationOutcome::ok(InvocationTarget::Command),
			Ok(CommandOutcome::Quit) => InvocationOutcome::quit(InvocationTarget::Command),
			Ok(CommandOutcome::ForceQuit) => InvocationOutcome::force_quit(InvocationTarget::Command),
			Err(error) => {
				let error = error.to_string();
				self.show_notification(xeno_registry::notifications::keys::command_error(&error));
				InvocationOutcome::command_error(InvocationTarget::Command, error)
			}
		};

		self.flush_effects();
		result
	}
}
