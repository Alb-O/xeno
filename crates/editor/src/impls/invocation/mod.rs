//! Unified invocation dispatch.
//!
//! Single entry point for user-invoked operations with consistent capability
//! checking, hook emission, and error handling.

use std::time::{Duration, Instant};

use nu_protocol::Value;
use tracing::{debug, trace, trace_span, warn};
use xeno_registry::actions::{ActionArgs, ActionContext, ActionResult, EditorContext, dispatch_result, find_action};
use xeno_registry::commands::{CommandContext, find_command};
use xeno_registry::hooks::{HookContext, emit_sync_with as emit_hook_sync_with};
use xeno_registry::{CommandError, HookEventData, RegistryEntry};

use crate::commands::{CommandOutcome, EditorCommandContext, find_editor_command};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

const MAX_NU_MACRO_DEPTH: u8 = 8;
const SLOW_NU_HOOK_THRESHOLD: Duration = Duration::from_millis(2);
const SLOW_NU_MACRO_THRESHOLD: Duration = Duration::from_millis(5);

/// Build hook args for action post hooks: `[name, result_label]`.
pub(crate) fn action_post_args(name: String, result: &InvocationResult) -> Vec<String> {
	vec![name, invocation_result_label(result).to_string()]
}

/// Build hook args for command/editor-command post hooks: `[name, result_label, ...original_args]`.
pub(crate) fn command_post_args(name: String, result: &InvocationResult, args: Vec<String>) -> Vec<String> {
	let mut hook_args = vec![name, invocation_result_label(result).to_string()];
	hook_args.extend(args);
	hook_args
}

/// Build hook args for mode change: `[from_debug, to_debug]`.
pub(crate) fn mode_change_args(from: &xeno_primitives::Mode, to: &xeno_primitives::Mode) -> Vec<String> {
	vec![format!("{from:?}"), format!("{to:?}")]
}

/// Build hook args for buffer open: `[path, kind]`.
pub(crate) fn buffer_open_args(path: &std::path::Path, kind: &str) -> Vec<String> {
	vec![path.to_string_lossy().to_string(), kind.to_string()]
}

#[cfg(test)]
mod tests;

impl Editor {
	/// Executes a named action with enforcement defaults.
	pub fn invoke_action(&mut self, name: &str, count: usize, extend: bool, register: Option<char>, char_arg: Option<char>) -> InvocationResult {
		self.run_action_invocation(name, count, extend, register, char_arg, InvocationPolicy::enforcing())
	}

	/// Executes a registry command with enforcement defaults.
	pub async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationResult {
		self.run_command_invocation(name, &args, InvocationPolicy::enforcing()).await
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
	/// Pre/post hooks are emitted for actions, commands, and editor commands.
	pub async fn run_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationResult {
		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		match invocation {
			Invocation::Action { name, count, extend, register } => {
				let result = self.run_action_invocation(&name, count, extend, register, None, policy);
				self.maybe_emit_post_hook(crate::nu::NuHook::ActionPost, action_post_args(name, &result), result)
					.await
			}

			Invocation::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => {
				let result = self.run_action_invocation(&name, count, extend, register, Some(char_arg), policy);
				self.maybe_emit_post_hook(crate::nu::NuHook::ActionPost, action_post_args(name, &result), result)
					.await
			}

			Invocation::Command { name, args } => {
				let result = self.run_command_invocation(&name, &args, policy).await;
				self.maybe_emit_post_hook(crate::nu::NuHook::CommandPost, command_post_args(name, &result, args), result)
					.await
			}

			Invocation::EditorCommand { name, args } => {
				let result = self.run_editor_command_invocation(&name, &args, policy).await;
				self.maybe_emit_post_hook(crate::nu::NuHook::EditorCommandPost, command_post_args(name, &result, args), result)
					.await
			}

			Invocation::Nu { name, args } => {
				if self.state.nu_macro_depth >= MAX_NU_MACRO_DEPTH {
					return InvocationResult::CommandError(format!("Nu macro recursion depth exceeded ({MAX_NU_MACRO_DEPTH})"));
				}

				self.state.nu_macro_depth += 1;
				let result = self.run_nu_macro_invocation(name, args, policy).await;
				self.state.nu_macro_depth = self.state.nu_macro_depth.saturating_sub(1);
				result
			}
		}
	}

	/// Run a Nu hook function if runtime is loaded.
	///
	/// Runs a Nu post-hook if the result is non-quit, propagating hook quit.
	async fn maybe_emit_post_hook(&mut self, hook: crate::nu::NuHook, args: Vec<String>, result: InvocationResult) -> InvocationResult {
		if !result.is_quit()
			&& let Some(hook_result) = self.run_nu_hook(hook, args).await
		{
			return hook_result;
		}
		result
	}

	/// Emits `on_action_post` hook for key-handling action dispatch path.
	pub(crate) async fn emit_action_post_hook(&mut self, name: String, result: &InvocationResult) -> Option<InvocationResult> {
		if result.is_quit() {
			return None;
		}
		self.run_nu_hook(crate::nu::NuHook::ActionPost, action_post_args(name, result)).await
	}

	/// Emits `on_mode_change` hook after a mode transition.
	pub(crate) async fn emit_mode_change_hook(&mut self, old: &xeno_primitives::Mode, new: &xeno_primitives::Mode) -> Option<InvocationResult> {
		self.run_nu_hook(crate::nu::NuHook::ModeChange, mode_change_args(old, new)).await
	}

	/// Emits `on_buffer_open` hook after a buffer is focused via navigation.
	pub(crate) async fn emit_buffer_open_hook(&mut self, path: &std::path::Path, kind: &str) -> Option<InvocationResult> {
		self.run_nu_hook(crate::nu::NuHook::BufferOpen, buffer_open_args(path, kind)).await
	}

	/// Hook errors are logged and ignored. Quit requests from hook-produced
	/// invocations are propagated to the caller.
	async fn run_nu_hook(&mut self, hook: crate::nu::NuHook, args: Vec<String>) -> Option<InvocationResult> {
		if self.state.nu_hook_guard {
			return None;
		}

		let fn_name = hook.fn_name();
		let decl_id = match hook {
			crate::nu::NuHook::ActionPost => self.state.nu_hook_ids.on_action_post,
			crate::nu::NuHook::CommandPost => self.state.nu_hook_ids.on_command_post,
			crate::nu::NuHook::EditorCommandPost => self.state.nu_hook_ids.on_editor_command_post,
			crate::nu::NuHook::ModeChange => self.state.nu_hook_ids.on_mode_change,
			crate::nu::NuHook::BufferOpen => self.state.nu_hook_ids.on_buffer_open,
		}?;
		self.ensure_nu_executor()?;

		let limits = self
			.state
			.config
			.nu
			.as_ref()
			.map_or_else(crate::nu::DecodeLimits::hook_defaults, |c| c.hook_decode_limits());
		let args_len = args.len();
		let hook_start = Instant::now();
		let nu_ctx = self.build_nu_ctx("hook", fn_name, &args);
		let hook_span = trace_span!(
			"nu.hook",
			function = fn_name,
			args_len = args_len,
			max_invocations = limits.max_invocations,
			max_depth = limits.max_depth
		);
		let _hook_guard = hook_span.enter();
		let env = vec![("XENO_CTX".to_string(), nu_ctx)];
		let invocations = match self.state.nu_executor.as_ref().unwrap().run(decl_id, args, limits, env).await {
			Ok(invocations) => invocations,
			Err(crate::nu::executor::NuExecError::Shutdown { decl_id, args, limits, env }) => {
				warn!(hook = fn_name, "Nu executor died, restarting");
				self.restart_nu_executor();
				match self.ensure_nu_executor() {
					Some(executor) => match executor.run(decl_id, args, limits, env).await {
						Ok(invocations) => invocations,
						Err(error) => {
							warn!(hook = fn_name, error = ?error, "Nu hook failed after executor restart");
							return None;
						}
					},
					None => return None,
				}
			}
			Err(crate::nu::executor::NuExecError::ReplyDropped) => {
				warn!(hook = fn_name, "Nu executor worker died mid-evaluation, restarting");
				self.restart_nu_executor();
				return None;
			}
			Err(crate::nu::executor::NuExecError::Eval(error)) => {
				warn!(hook = fn_name, error = %error, "Nu hook failed");
				return None;
			}
		};
		let hook_elapsed = hook_start.elapsed();
		if hook_elapsed > SLOW_NU_HOOK_THRESHOLD {
			debug!(hook = fn_name, elapsed_ms = hook_elapsed.as_millis() as u64, "slow Nu hook call");
		}

		self.state.nu_hook_guard = true;
		for invocation in invocations {
			let result = Box::pin(self.run_invocation(invocation, InvocationPolicy::enforcing())).await;

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

	async fn run_nu_macro_invocation(&mut self, fn_name: String, args: Vec<String>, policy: InvocationPolicy) -> InvocationResult {
		if let Err(error) = self.ensure_nu_runtime_loaded().await {
			self.show_notification(xeno_registry::notifications::keys::command_error(&error));
			return InvocationResult::CommandError(error);
		}

		let Some(runtime) = self.nu_runtime() else {
			return InvocationResult::CommandError("Nu runtime is not loaded".to_string());
		};

		let Some(decl_id) = runtime.find_script_decl(&fn_name) else {
			let error = format!("Nu runtime error: function '{}' is not defined in xeno.nu", fn_name);
			self.show_notification(xeno_registry::notifications::keys::command_error(&error));
			return InvocationResult::CommandError(error);
		};

		if self.ensure_nu_executor().is_none() {
			return InvocationResult::CommandError("Nu executor is not available".to_string());
		}

		let limits = self
			.state
			.config
			.nu
			.as_ref()
			.map_or_else(crate::nu::DecodeLimits::macro_defaults, |c| c.macro_decode_limits());
		let args_len = args.len();
		let macro_start = Instant::now();
		let nu_ctx = self.build_nu_ctx("macro", &fn_name, &args);
		let macro_span = trace_span!("nu.macro", function = %fn_name, args_len = args_len);
		let _macro_guard = macro_span.enter();
		let env = vec![("XENO_CTX".to_string(), nu_ctx)];

		let invocations = match self.state.nu_executor.as_ref().unwrap().run(decl_id, args, limits, env).await {
			Ok(invocations) => invocations,
			Err(crate::nu::executor::NuExecError::Shutdown { decl_id, args, limits, env }) => {
				warn!(function = %fn_name, "Nu executor died, restarting");
				self.restart_nu_executor();
				match self.ensure_nu_executor() {
					Some(executor) => match executor.run(decl_id, args, limits, env).await {
						Ok(invocations) => invocations,
						Err(error) => {
							let msg = exec_error_message(&error);
							self.show_notification(xeno_registry::notifications::keys::command_error(&msg));
							return InvocationResult::CommandError(msg);
						}
					},
					None => {
						let error = "Nu executor could not be restarted".to_string();
						self.show_notification(xeno_registry::notifications::keys::command_error(&error));
						return InvocationResult::CommandError(error);
					}
				}
			}
			Err(crate::nu::executor::NuExecError::ReplyDropped) => {
				warn!(function = %fn_name, "Nu executor worker died mid-evaluation, restarting");
				self.restart_nu_executor();
				let error = "Nu executor worker died during evaluation".to_string();
				self.show_notification(xeno_registry::notifications::keys::command_error(&error));
				return InvocationResult::CommandError(error);
			}
			Err(crate::nu::executor::NuExecError::Eval(error)) => {
				self.show_notification(xeno_registry::notifications::keys::command_error(&error));
				return InvocationResult::CommandError(error);
			}
		};
		let macro_elapsed = macro_start.elapsed();
		if macro_elapsed > SLOW_NU_MACRO_THRESHOLD {
			debug!(function = %fn_name, elapsed_ms = macro_elapsed.as_millis() as u64, "slow Nu macro call");
		}

		if invocations.is_empty() {
			debug!(function = %fn_name, "Nu macro produced no invocations");
			return InvocationResult::Ok;
		}

		for invocation in invocations {
			match Box::pin(self.run_invocation(invocation, policy)).await {
				InvocationResult::Ok => {}
				other => return other,
			}
		}

		InvocationResult::Ok
	}

	async fn ensure_nu_runtime_loaded(&mut self) -> Result<(), String> {
		if self.nu_runtime().is_some() {
			return Ok(());
		}

		let config_dir = crate::paths::get_config_dir().ok_or_else(|| "config directory is unavailable; cannot auto-load xeno.nu".to_string())?;
		let loaded = tokio::task::spawn_blocking(move || crate::nu::NuRuntime::load(&config_dir))
			.await
			.map_err(|error| format!("failed to join Nu runtime load task: {error}"))?;

		match loaded {
			Ok(runtime) => {
				self.set_nu_runtime(Some(runtime));
				Ok(())
			}
			Err(error) => Err(error),
		}
	}

	fn build_nu_ctx(&self, kind: &str, function: &str, args: &[String]) -> Value {
		use crate::nu::ctx::{NuCtx, NuCtxBuffer, NuCtxPosition, NuCtxSelection, NuCtxView};

		let buffer = self.buffer();
		let view_id = self.focused_view().0;
		let primary_selection = buffer.selection.primary();
		let cursor_char = buffer.cursor;

		let (cursor_line, cursor_col, sel_start_line, sel_start_col, sel_end_line, sel_end_col) = buffer.with_doc(|doc| {
			let text = doc.content();
			let to_line_col = |idx: usize| {
				let clamped = idx.min(text.len_chars());
				let line = text.char_to_line(clamped);
				let col = clamped.saturating_sub(text.line_to_char(line));
				(line, col)
			};

			let (cl, cc) = to_line_col(cursor_char);
			let (ssl, ssc) = to_line_col(primary_selection.min());
			let (sel, sec) = to_line_col(primary_selection.max());
			(cl, cc, ssl, ssc, sel, sec)
		});

		NuCtx {
			kind: kind.to_string(),
			function: function.to_string(),
			args: args.to_vec(),
			mode: format!("{:?}", self.mode()),
			view: NuCtxView { id: view_id },
			cursor: NuCtxPosition {
				line: cursor_line,
				col: cursor_col,
			},
			selection: NuCtxSelection {
				active: !primary_selection.is_point(),
				start: NuCtxPosition {
					line: sel_start_line,
					col: sel_start_col,
				},
				end: NuCtxPosition {
					line: sel_end_line,
					col: sel_end_col,
				},
			},
			buffer: NuCtxBuffer {
				path: buffer.path().map(|p| p.to_string_lossy().to_string()),
				file_type: buffer.file_type(),
				readonly: buffer.is_readonly(),
				modified: buffer.modified(),
			},
		}
		.to_value()
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

	async fn run_command_invocation(&mut self, name: &str, args: &[String], policy: InvocationPolicy) -> InvocationResult {
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
	async fn run_editor_command_invocation(&mut self, name: &str, args: &[String], policy: InvocationPolicy) -> InvocationResult {
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

fn invocation_result_label(result: &InvocationResult) -> &'static str {
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
	Command,
	EditorCommand,
}

fn notify_capability_denied(editor: &mut Editor, kind: InvocationKind, error: &CommandError) {
	match kind {
		InvocationKind::Action => editor.show_notification(xeno_registry::notifications::keys::action_error(error)),
		InvocationKind::Command => {
			let error = error.to_string();
			editor.show_notification(xeno_registry::notifications::keys::command_error(&error));
		}
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

fn exec_error_message(error: &crate::nu::executor::NuExecError) -> String {
	match error {
		crate::nu::executor::NuExecError::Shutdown { .. } => "Nu executor thread has shut down".to_string(),
		crate::nu::executor::NuExecError::ReplyDropped => "Nu executor worker died during evaluation".to_string(),
		crate::nu::executor::NuExecError::Eval(msg) => msg.clone(),
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
