//! Unified invocation dispatch.
//!
//! Single entry point for user-invoked operations with consistent capability
//! checking, hook emission, and error handling.

use tracing::{debug, trace, trace_span, warn};
use xeno_registry::actions::find_action;
use xeno_registry::commands::find_command;
use xeno_registry::{
	ActionArgs, ActionContext, CommandContext, EditorContext, HookContext, HookEventData,
	emit_sync_with as emit_hook_sync_with,
};

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

impl Editor {
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
				self.run_editor_command_invocation(&name, args, policy).await
			}
		}
	}

	fn run_action_invocation(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: Option<char>,
		policy: InvocationPolicy,
	) -> InvocationResult {
		let Some(action) = find_action(name) else {
			self.show_notification(xeno_registry_notifications::keys::unknown_action::call(name));
			return InvocationResult::NotFound(format!("action:{name}"));
		};

		let required_caps = action.required_caps();
		if !required_caps.is_empty() {
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(required_caps) {
				if policy.enforce_caps {
					self.show_notification(xeno_registry_notifications::keys::action_error::call(
						e.clone(),
					));
					let cap_str = e.to_string();
					return parse_capability_from_error(&cap_str)
						.map(InvocationResult::CapabilityDenied)
						.unwrap_or_else(|| InvocationResult::CommandError(cap_str));
				}
				warn!(action = name, error = %e, "Capability check failed (log-only mode)");
			}
		}

		if policy.enforce_readonly
			&& requires_edit_capability(required_caps)
			&& self.buffer().is_readonly()
		{
			self.show_notification(xeno_registry_notifications::keys::buffer_readonly.into());
			return InvocationResult::ReadonlyDenied;
		}

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::ActionPre {
					action_id: action.id(),
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
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
		if !required_caps.is_empty() {
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(required_caps) {
				if policy.enforce_caps {
					self.show_notification(xeno_registry_notifications::keys::command_error::call(
						&e.to_string(),
					));
					let cap_str = e.to_string();
					return parse_capability_from_error(&cap_str)
						.map(InvocationResult::CapabilityDenied)
						.unwrap_or_else(|| InvocationResult::CommandError(cap_str));
				}
				warn!(command = name, error = %e, "Command capability check failed (log-only mode)");
			}
		}

		if policy.enforce_readonly
			&& requires_edit_capability(required_caps)
			&& self.buffer().is_readonly()
		{
			self.show_notification(xeno_registry_notifications::keys::buffer_readonly.into());
			return InvocationResult::ReadonlyDenied;
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
				self.show_notification(xeno_registry_notifications::keys::command_error::call(
					&e.to_string(),
				));
				InvocationResult::CommandError(e.to_string())
			}
		}
	}

	/// Editor commands lack capability metadata; capability gating not yet supported.
	async fn run_editor_command_invocation(
		&mut self,
		name: &str,
		args: Vec<String>,
		_policy: InvocationPolicy,
	) -> InvocationResult {
		let Some(editor_cmd) = find_editor_command(name) else {
			return InvocationResult::NotFound(format!("editor_command:{name}"));
		};

		debug!(command = name, "Executing editor command");

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
				self.show_notification(xeno_registry_notifications::keys::command_error::call(
					&e.to_string(),
				));
				InvocationResult::CommandError(e.to_string())
			}
		}
	}
}

fn requires_edit_capability(caps: &[xeno_registry::Capability]) -> bool {
	caps.iter()
		.any(|c| matches!(c, xeno_registry::Capability::Edit))
}

/// Parses capability name from error message format "missing capability: <cap>".
fn parse_capability_from_error(error: &str) -> Option<xeno_registry::Capability> {
	use xeno_registry::Capability;
	let cap_str = error.strip_prefix("missing capability: ")?.trim();
	match cap_str {
		"Text" => Some(Capability::Text),
		"Cursor" => Some(Capability::Cursor),
		"Selection" => Some(Capability::Selection),
		"Mode" => Some(Capability::Mode),
		"Messaging" => Some(Capability::Messaging),
		"Edit" => Some(Capability::Edit),
		"Search" => Some(Capability::Search),
		"Undo" => Some(Capability::Undo),
		"FileOps" => Some(Capability::FileOps),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn invocation_describe() {
		assert_eq!(Invocation::action("move_left").describe(), "action:move_left");
		assert_eq!(
			Invocation::action_with_count("move_down", 5).describe(),
			"action:move_downx5"
		);
		assert_eq!(
			Invocation::command("write", vec!["file.txt".into()]).describe(),
			"cmd:write file.txt"
		);
		assert_eq!(
			Invocation::editor_command("quit", vec![]).describe(),
			"editor_cmd:quit"
		);
	}

	#[test]
	fn invocation_policy_defaults() {
		let policy = InvocationPolicy::default();
		assert!(!policy.enforce_caps);
		assert!(!policy.enforce_readonly);

		let policy = InvocationPolicy::enforcing();
		assert!(policy.enforce_caps);
		assert!(policy.enforce_readonly);
	}
}
