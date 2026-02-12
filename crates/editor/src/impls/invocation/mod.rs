//! Unified invocation dispatch.
//!
//! Single entry point for user-invoked operations with consistent capability
//! checking, hook emission, and error handling.

use tracing::{debug, trace, trace_span, warn};
use xeno_registry::actions::{ActionArgs, ActionContext, ActionResult, EditorContext, dispatch_result, find_action};
use xeno_registry::commands::{CommandContext, find_command};
use xeno_registry::hooks::{HookContext, emit_sync_with as emit_hook_sync_with};
use xeno_registry::{CommandError, HookEventData, RegistryEntry};

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

#[cfg(test)]
mod tests;

impl Editor {
	/// Executes a named action with enforcement defaults.
	pub fn invoke_action(&mut self, name: &str, count: usize, extend: bool, register: Option<char>, char_arg: Option<char>) -> InvocationResult {
		self.run_action_invocation(name, count, extend, register, char_arg, InvocationPolicy::enforcing())
	}

	/// Executes a registry command with enforcement defaults.
	pub async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationResult {
		self.run_command_invocation(name, args, InvocationPolicy::enforcing()).await
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
	pub async fn run_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationResult {
		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		match invocation {
			Invocation::Action { name, count, extend, register } => {
				let result = self.run_action_invocation(&name, count, extend, register, None, policy);
				if !result.is_quit() {
					if let Some(hook_result) = self.run_nu_hook("on_action_post", vec![name, action_result_label(&result).to_string()]).await {
						return hook_result;
					}
				}
				result
			}

			Invocation::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => {
				let result = self.run_action_invocation(&name, count, extend, register, Some(char_arg), policy);
				if !result.is_quit() {
					if let Some(hook_result) = self.run_nu_hook("on_action_post", vec![name, action_result_label(&result).to_string()]).await {
						return hook_result;
					}
				}
				result
			}

			Invocation::Command { name, args } => self.run_command_invocation(&name, args, policy).await,

			Invocation::EditorCommand { name, args } => self.run_editor_command_invocation(&name, args, policy).await,
		}
	}

	/// Run a Nu hook function if runtime is loaded.
	///
	/// Hook errors are logged and ignored. Quit requests from hook-produced
	/// invocations are propagated to the caller.
	pub async fn run_nu_hook(&mut self, fn_name: &str, args: Vec<String>) -> Option<InvocationResult> {
		if self.state.nu_hook_guard {
			return None;
		}

		let runtime = self.nu_runtime()?.clone();
		let fn_name_owned = fn_name.to_string();
		let invocations = match tokio::task::spawn_blocking(move || runtime.try_run_invocations(&fn_name_owned, &args)).await {
			Ok(Ok(Some(invocations))) => invocations,
			Ok(Ok(None)) => return None,
			Ok(Err(error)) => {
				warn!(hook = fn_name, error = %error, "Nu hook failed");
				return None;
			}
			Err(error) => {
				warn!(hook = fn_name, error = %error, "Nu hook task join failed");
				return None;
			}
		};

		self.state.nu_hook_guard = true;
		for invocation in invocations {
			let result = match invocation {
				Invocation::Action { name, count, extend, register } => {
					self.run_action_invocation(&name, count, extend, register, None, InvocationPolicy::enforcing())
				}
				Invocation::ActionWithChar {
					name,
					count,
					extend,
					register,
					char_arg,
				} => self.run_action_invocation(&name, count, extend, register, Some(char_arg), InvocationPolicy::enforcing()),
				Invocation::Command { name, args } => self.run_command_invocation(&name, args, InvocationPolicy::enforcing()).await,
				Invocation::EditorCommand { name, args } => self.run_editor_command_invocation(&name, args, InvocationPolicy::enforcing()).await,
			};

			match result {
				InvocationResult::Ok => {}
				InvocationResult::Quit => {
					self.state.nu_hook_guard = false;
					return Some(InvocationResult::Quit);
				}
				InvocationResult::ForceQuit => {
					self.state.nu_hook_guard = false;
					return Some(InvocationResult::ForceQuit);
				}
				InvocationResult::NotFound(target) => {
					warn!(hook = fn_name, target = %target, "Nu hook invocation not found");
				}
				InvocationResult::CapabilityDenied(cap) => {
					warn!(hook = fn_name, capability = ?cap, "Nu hook invocation denied by capability");
				}
				InvocationResult::ReadonlyDenied => {
					warn!(hook = fn_name, "Nu hook invocation denied by readonly mode");
				}
				InvocationResult::CommandError(error) => {
					warn!(hook = fn_name, error = %error, "Nu hook invocation failed");
				}
			}
		}

		self.state.nu_hook_guard = false;
		None
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
			let mut caps = self.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
			e_ctx.check_capability_set(required_caps).err()
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

		if policy.enforce_readonly && requires_edit_capability_set(required_caps) && self.buffer().is_readonly() {
			return notify_readonly_denied(self);
		}

		let action_id_str = action.id_str().to_string();
		let action_name_str = action.name_str().to_string();

		emit_hook_sync_with(
			&HookContext::new(HookEventData::ActionPre { action_id: &action_id_str }),
			&mut self.state.hook_runtime,
		);

		let span = trace_span!(
			"action",
			name = %action_name_str,
			id = %action_id_str,
			count = count,
			extend = extend,
		);
		let _guard = span.enter();

		self.buffer_mut().ensure_valid_selection();
		let (content, cursor, selection) = {
			let buffer = self.buffer();
			(buffer.with_doc(|doc| doc.content().clone()), buffer.cursor, buffer.selection.clone())
		};

		let handler = action.handler;

		let ctx = ActionContext {
			text: content.slice(..),
			cursor,
			selection: &selection,
			count,
			extend,
			register,
			args: ActionArgs { char: char_arg, string: None },
		};

		let result = handler(&ctx);
		trace!(result = ?result, "Action completed");

		let outcome = if self.apply_action_result(&action_id_str, result, extend) {
			InvocationResult::Quit
		} else {
			InvocationResult::Ok
		};

		self.flush_effects();
		outcome
	}

	async fn run_command_invocation(&mut self, name: &str, args: Vec<String>, policy: InvocationPolicy) -> InvocationResult {
		let Some(command_def) = find_command(name) else {
			// Don't notify - caller may want to try editor commands next
			return InvocationResult::NotFound(format!("command:{name}"));
		};

		let required_caps = command_def.required_caps();
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut caps = self.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
			e_ctx.check_capability_set(required_caps).err()
		};
		if let Some(e) = caps_error
			&& let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| notify_capability_denied(self, InvocationKind::Action, err),
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

		if policy.enforce_readonly && requires_edit_capability_set(required_caps) && self.buffer().is_readonly() {
			return notify_readonly_denied(self);
		}

		let handler = command_def.handler;
		let user_data = command_def.user_data;

		let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
		let res = {
			let mut caps = self.caps();
			let mut ctx = CommandContext {
				editor: &mut caps,
				args: &args_refs,
				count: 1,
				register: None,
				user_data,
			};

			handler(&mut ctx).await
		};

		let outcome = match res {
			Ok(CommandOutcome::Ok) => InvocationResult::Ok,
			Ok(CommandOutcome::Quit) => InvocationResult::Quit,
			Ok(CommandOutcome::ForceQuit) => InvocationResult::ForceQuit,
			Err(e) => {
				self.show_notification(xeno_registry::notifications::keys::command_error(&e.to_string()));
				InvocationResult::CommandError(e.to_string())
			}
		};

		self.flush_effects();
		outcome
	}

	/// Executes editor-direct commands with capability gating and policy checks.
	async fn run_editor_command_invocation(&mut self, name: &str, args: Vec<String>, policy: InvocationPolicy) -> InvocationResult {
		let Some(editor_cmd) = find_editor_command(name) else {
			return InvocationResult::NotFound(format!("editor_command:{name}"));
		};

		debug!(command = name, "Executing editor command");

		let required_caps = editor_cmd.required_caps;
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut caps = self.caps();
			let mut e_ctx = EditorContext::new(&mut caps);
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

		if policy.enforce_readonly && requires_edit_capability(required_caps) && self.buffer().is_readonly() {
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

		let outcome = match (editor_cmd.handler)(&mut ctx).await {
			Ok(CommandOutcome::Ok) => InvocationResult::Ok,
			Ok(CommandOutcome::Quit) => InvocationResult::Quit,
			Ok(CommandOutcome::ForceQuit) => InvocationResult::ForceQuit,
			Err(e) => {
				self.show_notification(xeno_registry::notifications::keys::command_error(&e.to_string()));
				InvocationResult::CommandError(e.to_string())
			}
		};

		self.flush_effects();
		outcome
	}

	/// Dispatches an action result to handlers and emits post-action hook.
	pub(crate) fn apply_action_result(&mut self, action_id: &str, result: ActionResult, extend: bool) -> bool {
		let (should_quit, result_variant) = {
			let mut caps = self.caps();
			let mut ctx = EditorContext::new(&mut caps);
			let result_variant = result.variant_name();
			let should_quit = dispatch_result(&result, &mut ctx, extend);
			(should_quit, result_variant)
		};

		emit_hook_sync_with(
			&HookContext::new(HookEventData::ActionPost { action_id, result_variant }),
			&mut self.state.hook_runtime,
		);
		should_quit
	}
}

fn action_result_label(result: &InvocationResult) -> &'static str {
	match result {
		InvocationResult::Ok => "ok",
		InvocationResult::Quit => "quit",
		InvocationResult::ForceQuit => "force_quit",
		InvocationResult::NotFound(_) => "not_found",
		InvocationResult::CapabilityDenied(_) => "cap_denied",
		InvocationResult::ReadonlyDenied => "readonly",
		InvocationResult::CommandError(_) => "error",
	}
}

enum InvocationKind {
	Action,
	EditorCommand,
}

fn notify_capability_denied(editor: &mut Editor, kind: InvocationKind, error: &CommandError) {
	match kind {
		InvocationKind::Action => editor.show_notification(xeno_registry::notifications::keys::action_error(error)),
		InvocationKind::EditorCommand => {
			let error = error.to_string();
			editor.show_notification(xeno_registry::notifications::keys::command_error(&error));
		}
	}
}

fn notify_readonly_denied(editor: &mut Editor) -> InvocationResult {
	editor.show_notification(xeno_registry::notifications::keys::BUFFER_READONLY.into());
	InvocationResult::ReadonlyDenied
}

fn requires_edit_capability_set(caps: xeno_registry::CapabilitySet) -> bool {
	caps.contains(xeno_registry::CapabilitySet::EDIT)
}

fn requires_edit_capability(caps: &[xeno_registry::Capability]) -> bool {
	caps.iter().any(|c| matches!(c, xeno_registry::Capability::Edit))
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
