//! Unified invocation dispatch.
//!
//! Single entry point for user-invoked operations with consistent capability
//! checking, hook emission, and error handling.

use tracing::{debug, trace, trace_span, warn};
use xeno_registry::actions::find_action;
use xeno_registry::commands::find_command;
use xeno_registry::{
	ActionArgs, ActionContext, ActionResult, CommandContext, CommandError, EditorContext,
	HookContext, HookEventData, dispatch_result, emit_sync_with as emit_hook_sync_with,
};

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

#[cfg(test)]
mod tests;

impl Editor {
	/// Executes a named action with enforcement defaults.
	pub fn invoke_action(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: Option<char>,
	) -> InvocationResult {
		self.run_action_invocation(
			name,
			count,
			extend,
			register,
			char_arg,
			InvocationPolicy::enforcing(),
		)
	}

	/// Executes a registry command with enforcement defaults.
	pub async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationResult {
		self.run_command_invocation(name, args, InvocationPolicy::enforcing())
			.await
	}

	/// Executes an invocation with capability gating and hook emission.
	///
	/// Unified entry point for keymap dispatch, command palette, ex commands,
	/// and hook-triggered invocations.
	///
	/// # Capability Enforcement
	///
	/// When `policy.enforce_caps` is true, missing capabilities block execution
	/// and return [`InvocationResult::CapabilityDenied`]. When false, violations
	/// are logged but execution continues (log-only mode for migration).
	///
	/// # Hook Emission
	///
	/// Pre/post hooks are emitted for actions. Command hooks may be added later.
	pub async fn run_invocation(
		&mut self,
		invocation: Invocation,
		policy: InvocationPolicy,
	) -> InvocationResult {
		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		match invocation {
			Invocation::Action {
				name,
				count,
				extend,
				register,
			} => self.run_action_invocation(&name, count, extend, register, None, policy),

			Invocation::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => self.run_action_invocation(&name, count, extend, register, Some(char_arg), policy),

			Invocation::Command { name, args } => {
				self.run_command_invocation(&name, args, policy).await
			}

			Invocation::EditorCommand { name, args } => {
				self.run_editor_command_invocation(&name, args, policy)
					.await
			}
		}
	}

	pub(crate) fn run_action_invocation(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: Option<char>,
		policy: InvocationPolicy,
	) -> InvocationResult {
		let Some(action) = find_action(name) else {
			self.show_notification(xeno_registry::notifications::keys::unknown_action(name));
			return InvocationResult::NotFound(format!("action:{name}"));
		};

		let required_caps = action.required_caps();
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut e_ctx = EditorContext::new(self);
			e_ctx.check_all_capabilities(required_caps).err()
		};
		if let Some(e) = caps_error
			&& let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| notify_capability_denied(self, InvocationKind::Action, err),
				|err| warn!(action = name, error = %err, "Capability check failed (log-only mode)"),
			) {
			return result;
		}

		if policy.enforce_readonly
			&& requires_edit_capability(required_caps)
			&& self.buffer().is_readonly()
		{
			return notify_readonly_denied(self);
		}

		emit_hook_sync_with(
			&HookContext::new(HookEventData::ActionPre {
				action_id: action.id(),
			}),
			&mut self.state.hook_runtime,
		);

		let span = trace_span!(
			"action",
			name = action.name(),
			id = action.id(),
			count = count,
			extend = extend,
		);
		let _guard = span.enter();

		self.buffer_mut().ensure_valid_selection();
		let (content, cursor, selection) = {
			let buffer = self.buffer();
			(
				buffer.with_doc(|doc| doc.content().clone()),
				buffer.cursor,
				buffer.selection.clone(),
			)
		};

		let ctx = ActionContext {
			text: content.slice(..),
			cursor,
			selection: &selection,
			count,
			extend,
			register,
			args: ActionArgs {
				char: char_arg,
				string: None,
			},
		};

		let result = (action.handler)(&ctx);
		trace!(result = ?result, "Action completed");

		if self.apply_action_result(action.id(), result, extend) {
			InvocationResult::Quit
		} else {
			InvocationResult::Ok
		}
	}

	async fn run_command_invocation(
		&mut self,
		name: &str,
		args: Vec<String>,
		policy: InvocationPolicy,
	) -> InvocationResult {
		let Some(command_def) = find_command(name) else {
			// Don't notify - caller may want to try editor commands next
			return InvocationResult::NotFound(format!("command:{name}"));
		};

		let required_caps = command_def.required_caps();
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut e_ctx = EditorContext::new(self);
			e_ctx.check_all_capabilities(required_caps).err()
		};
		if let Some(e) = caps_error
			&& let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| notify_capability_denied(self, InvocationKind::Command, err),
				|err| {
					warn!(
						command = name,
						error = %err,
						"Command capability check failed (log-only mode)"
					);
				},
			) {
			return result;
		}

		if policy.enforce_readonly
			&& requires_edit_capability(required_caps)
			&& self.buffer().is_readonly()
		{
			return notify_readonly_denied(self);
		}

		let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
		let mut ctx = CommandContext {
			editor: self,
			args: &args_refs,
			count: 1,
			register: None,
			user_data: command_def.user_data,
		};

		match (command_def.handler)(&mut ctx).await {
			Ok(CommandOutcome::Ok) => InvocationResult::Ok,
			Ok(CommandOutcome::Quit) => InvocationResult::Quit,
			Ok(CommandOutcome::ForceQuit) => InvocationResult::ForceQuit,
			Err(e) => {
				self.show_notification(xeno_registry::notifications::keys::command_error(
					&e.to_string(),
				));
				InvocationResult::CommandError(e.to_string())
			}
		}
	}

	/// Executes editor-direct commands with capability gating and policy checks.
	async fn run_editor_command_invocation(
		&mut self,
		name: &str,
		args: Vec<String>,
		policy: InvocationPolicy,
	) -> InvocationResult {
		let Some(editor_cmd) = find_editor_command(name) else {
			return InvocationResult::NotFound(format!("editor_command:{name}"));
		};

		debug!(command = name, "Executing editor command");

		let required_caps = editor_cmd.required_caps;
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut e_ctx = EditorContext::new(self);
			e_ctx.check_all_capabilities(required_caps).err()
		};
		if let Some(e) = caps_error
			&& let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| notify_capability_denied(self, InvocationKind::EditorCommand, err),
				|err| {
					warn!(
						command = name,
						error = %err,
						"Command capability check failed (log-only mode)"
					);
				},
			) {
			return result;
		}

		if policy.enforce_readonly
			&& requires_edit_capability(required_caps)
			&& self.buffer().is_readonly()
		{
			return notify_readonly_denied(self);
		}

		let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
		let mut ctx = EditorCommandContext {
			editor: self,
			args: &args_refs,
			count: 1,
			register: None,
			user_data: editor_cmd.user_data,
		};

		match (editor_cmd.handler)(&mut ctx).await {
			Ok(CommandOutcome::Ok) => InvocationResult::Ok,
			Ok(CommandOutcome::Quit) => InvocationResult::Quit,
			Ok(CommandOutcome::ForceQuit) => InvocationResult::ForceQuit,
			Err(e) => {
				self.show_notification(xeno_registry::notifications::keys::command_error(
					&e.to_string(),
				));
				InvocationResult::CommandError(e.to_string())
			}
		}
	}

	/// Dispatches an action result to handlers and emits post-action hook.
	pub(crate) fn apply_action_result(
		&mut self,
		action_id: &'static str,
		result: ActionResult,
		extend: bool,
	) -> bool {
		let mut ctx = EditorContext::new(self);
		let result_variant = result.variant_name();
		let should_quit = dispatch_result(&result, &mut ctx, extend);
		emit_hook_sync_with(
			&HookContext::new(HookEventData::ActionPost {
				action_id,
				result_variant,
			}),
			&mut self.state.hook_runtime,
		);
		should_quit
	}
}

enum InvocationKind {
	Action,
	Command,
	EditorCommand,
}

fn notify_capability_denied(editor: &mut Editor, kind: InvocationKind, error: &CommandError) {
	match kind {
		InvocationKind::Action => {
			editor.show_notification(xeno_registry::notifications::keys::action_error(error))
		}
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

fn requires_edit_capability(caps: &[xeno_registry::Capability]) -> bool {
	caps.iter()
		.any(|c| matches!(c, xeno_registry::Capability::Edit))
}

fn capability_error_result(error: &CommandError) -> InvocationResult {
	match error {
		CommandError::MissingCapability(cap) => InvocationResult::CapabilityDenied(*cap),
		_ => InvocationResult::CommandError(error.to_string()),
	}
}

fn handle_capability_violation(
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
