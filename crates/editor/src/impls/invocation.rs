//! Unified invocation dispatch.
//!
//! Single entry point for user-invoked operations with consistent capability
//! checking, hook emission, and error handling.

use tracing::{debug, trace, trace_span, warn};
use xeno_registry::actions::find_action;
use xeno_registry::commands::find_command;
use xeno_registry::{
	ActionArgs, ActionContext, CommandContext, CommandError, EditorContext, HookContext,
	HookEventData, emit_sync_with as emit_hook_sync_with,
};

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

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
	pub async fn invoke_command(
		&mut self,
		name: &str,
		args: Vec<String>,
	) -> InvocationResult {
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
			self.show_notification(xeno_registry_notifications::keys::unknown_action::call(
				name,
			));
			return InvocationResult::NotFound(format!("action:{name}"));
		};

		let required_caps = action.required_caps();
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut e_ctx = EditorContext::new(self);
			e_ctx.check_all_capabilities(required_caps).err()
		};
		if let Some(e) = caps_error {
			if let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| {
					self.show_notification(
						xeno_registry_notifications::keys::action_error::call(err),
					);
				},
				|err| warn!(action = name, error = %err, "Capability check failed (log-only mode)"),
			) {
				return result;
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
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut e_ctx = EditorContext::new(self);
			e_ctx.check_all_capabilities(required_caps).err()
		};
		if let Some(e) = caps_error {
			if let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| {
					let error = err.to_string();
					self.show_notification(
						xeno_registry_notifications::keys::command_error::call(&error),
					);
				},
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
		policy: InvocationPolicy,
	) -> InvocationResult {
		let Some(editor_cmd) = find_editor_command(name) else {
			return InvocationResult::NotFound(format!("editor_command:{name}"));
		};

		debug!(command = name, "Executing editor command");

		let required_caps: &[xeno_registry::Capability] = &[];
		let caps_error = if required_caps.is_empty() {
			None
		} else {
			let mut e_ctx = EditorContext::new(self);
			e_ctx.check_all_capabilities(required_caps).err()
		};
		if let Some(e) = caps_error {
			if let Some(result) = handle_capability_violation(
				policy,
				e,
				|err| {
					let error = err.to_string();
					self.show_notification(
						xeno_registry_notifications::keys::command_error::call(&error),
					);
				},
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
		}

		if policy.enforce_readonly
			&& requires_edit_capability(required_caps)
			&& self.buffer().is_readonly()
		{
			self.show_notification(xeno_registry_notifications::keys::buffer_readonly.into());
			return InvocationResult::ReadonlyDenied;
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

#[cfg(test)]
mod tests {
	use std::cell::Cell;

	use xeno_primitives::range::CharIdx;
	use xeno_primitives::{Mode, Selection};
	use xeno_registry::{
		ActionEffects, ActionResult, Capability, CursorAccess, EditorCapabilities, ModeAccess,
		Notification, NotificationAccess, SelectionAccess, action, hook, HookAction, HookEventData,
	};

	use super::*;

	thread_local! {
		static ACTION_PRE_COUNT: Cell<usize> = Cell::new(0);
		static ACTION_POST_COUNT: Cell<usize> = Cell::new(0);
	}

	action!(
		invocation_test_action,
		{ description: "Invocation test action" },
		|_ctx| ActionResult::Effects(ActionEffects::ok())
	);

	hook!(
		invocation_test_action_pre,
		ActionPre,
		0,
		"Count action pre hooks",
		|ctx| {
			if let HookEventData::ActionPre { .. } = &ctx.data {
				ACTION_PRE_COUNT.with(|count| count.set(count.get() + 1));
			}
			HookAction::done()
		}
	);

	hook!(
		invocation_test_action_post,
		ActionPost,
		0,
		"Count action post hooks",
		|ctx| {
			if let HookEventData::ActionPost { .. } = &ctx.data {
				ACTION_POST_COUNT.with(|count| count.set(count.get() + 1));
			}
			HookAction::done()
		}
	);

	struct MockEditor {
		cursor: CharIdx,
		selection: Selection,
		mode: Mode,
		notifications: Vec<Notification>,
	}

	impl MockEditor {
		fn new() -> Self {
			Self {
				cursor: CharIdx::from(0usize),
				selection: Selection::point(CharIdx::from(0usize)),
				mode: Mode::Normal,
				notifications: Vec::new(),
			}
		}
	}

	impl CursorAccess for MockEditor {
		fn cursor(&self) -> CharIdx {
			self.cursor
		}

		fn cursor_line_col(&self) -> Option<(usize, usize)> {
			Some((0, usize::from(self.cursor)))
		}

		fn set_cursor(&mut self, pos: CharIdx) {
			self.cursor = pos;
		}
	}

	impl SelectionAccess for MockEditor {
		fn selection(&self) -> &Selection {
			&self.selection
		}

		fn selection_mut(&mut self) -> &mut Selection {
			&mut self.selection
		}

		fn set_selection(&mut self, sel: Selection) {
			self.selection = sel;
		}
	}

	impl ModeAccess for MockEditor {
		fn mode(&self) -> Mode {
			self.mode.clone()
		}

		fn set_mode(&mut self, mode: Mode) {
			self.mode = mode;
		}
	}

	impl NotificationAccess for MockEditor {
		fn emit(&mut self, notification: Notification) {
			self.notifications.push(notification);
		}

		fn clear_notifications(&mut self) {
			self.notifications.clear();
		}
	}

	impl EditorCapabilities for MockEditor {}

	#[test]
	fn invocation_describe() {
		assert_eq!(
			Invocation::action("move_left").describe(),
			"action:move_left"
		);
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

	#[test]
	fn capability_enforcement_blocks_when_enforced() {
		let mut editor = MockEditor::new();
		let mut ctx = EditorContext::new(&mut editor);
		let error = ctx
			.check_all_capabilities(&[Capability::Search])
			.expect_err("expected missing capability");

		let notified = Cell::new(false);
		let logged = Cell::new(false);

		let result = handle_capability_violation(
			InvocationPolicy::enforcing(),
			error,
			|_err| notified.set(true),
			|_err| logged.set(true),
		);

		assert!(notified.get());
		assert!(!logged.get());
		assert!(matches!(
			result,
			Some(InvocationResult::CapabilityDenied(Capability::Search))
		));
	}

	#[test]
	fn capability_enforcement_logs_in_log_only_mode() {
		let mut editor = MockEditor::new();
		let mut ctx = EditorContext::new(&mut editor);
		let error = ctx
			.check_all_capabilities(&[Capability::Search])
			.expect_err("expected missing capability");

		let notified = Cell::new(false);
		let logged = Cell::new(false);

		let result = handle_capability_violation(
			InvocationPolicy::log_only(),
			error,
			|_err| notified.set(true),
			|_err| logged.set(true),
		);

		assert!(result.is_none());
		assert!(!notified.get());
		assert!(logged.get());
	}

	#[test]
	fn action_hooks_fire_once() {
		ACTION_PRE_COUNT.with(|count| count.set(0));
		ACTION_POST_COUNT.with(|count| count.set(0));

		let mut editor = Editor::new_scratch();
		let result = editor.invoke_action("invocation_test_action", 1, false, None, None);
		assert!(matches!(result, InvocationResult::Ok));

		let pre_count = ACTION_PRE_COUNT.with(|count| count.get());
		let post_count = ACTION_POST_COUNT.with(|count| count.get());

		assert_eq!(pre_count, 1);
		assert_eq!(post_count, 1);
	}
}
