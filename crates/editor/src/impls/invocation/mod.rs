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

/// Maximum pending Nu hooks before oldest are dropped.
const MAX_PENDING_NU_HOOKS: usize = 64;
/// Maximum Nu hooks drained per pump() cycle.
pub(crate) const MAX_NU_HOOKS_PER_PUMP: usize = 2;

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
				if !result.is_quit() {
					self.enqueue_nu_hook(crate::nu::NuHook::ActionPost, action_post_args(name, &result));
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
					self.enqueue_nu_hook(crate::nu::NuHook::ActionPost, action_post_args(name, &result));
				}
				result
			}

			Invocation::Command { name, args } => {
				let result = self.run_command_invocation(&name, &args, policy).await;
				if !result.is_quit() {
					self.enqueue_nu_hook(crate::nu::NuHook::CommandPost, command_post_args(name, &result, args));
				}
				result
			}

			Invocation::EditorCommand { name, args } => {
				let result = self.run_editor_command_invocation(&name, &args, policy).await;
				if !result.is_quit() {
					self.enqueue_nu_hook(crate::nu::NuHook::EditorCommandPost, command_post_args(name, &result, args));
				}
				result
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

	/// Enqueues a Nu post-hook for deferred evaluation during pump().
	///
	/// Coalesces consecutive identical hook types (keeps latest args) and
	/// drops the oldest entry when the queue exceeds `MAX_PENDING_NU_HOOKS`.
	fn enqueue_nu_hook(&mut self, hook: crate::nu::NuHook, args: Vec<String>) {
		// Don't enqueue during hook drain (prevents recursive hook chains).
		if self.state.nu_hook_depth > 0 {
			return;
		}

		// Skip if the hook function isn't defined.
		let has_decl = match hook {
			crate::nu::NuHook::ActionPost => self.state.nu_hook_ids.on_action_post.is_some(),
			crate::nu::NuHook::CommandPost => self.state.nu_hook_ids.on_command_post.is_some(),
			crate::nu::NuHook::EditorCommandPost => self.state.nu_hook_ids.on_editor_command_post.is_some(),
			crate::nu::NuHook::ModeChange => self.state.nu_hook_ids.on_mode_change.is_some(),
			crate::nu::NuHook::BufferOpen => self.state.nu_hook_ids.on_buffer_open.is_some(),
		};
		if !has_decl {
			return;
		}

		// Coalesce: if back of queue is the same hook type, replace args and reset retries.
		if let Some(back) = self.state.nu_hook_queue.back_mut() {
			if back.hook == hook {
				back.args = args;
				back.retries = 0;
				return;
			}
		}

		// Backlog cap: drop oldest if full.
		if self.state.nu_hook_queue.len() >= MAX_PENDING_NU_HOOKS {
			self.state.nu_hook_queue.pop_front();
			self.state.nu_hook_dropped_total += 1;
			trace!(
				queue_len = self.state.nu_hook_queue.len(),
				dropped_total = self.state.nu_hook_dropped_total,
				"nu_hook.drop_oldest"
			);
		}

		self.state.nu_hook_queue.push_back(super::QueuedNuHook { hook, args, retries: 0 });
	}

	/// Enqueues `on_action_post` hook for key-handling action dispatch path.
	pub(crate) fn enqueue_action_post_hook(&mut self, name: String, result: &InvocationResult) {
		if !result.is_quit() {
			self.enqueue_nu_hook(crate::nu::NuHook::ActionPost, action_post_args(name, result));
		}
	}

	/// Enqueues `on_mode_change` hook after a mode transition.
	pub(crate) fn enqueue_mode_change_hook(&mut self, old: &xeno_primitives::Mode, new: &xeno_primitives::Mode) {
		self.enqueue_nu_hook(crate::nu::NuHook::ModeChange, mode_change_args(old, new));
	}

	/// Enqueues `on_buffer_open` hook after a buffer is focused via navigation.
	pub(crate) fn enqueue_buffer_open_hook(&mut self, path: &std::path::Path, kind: &str) {
		self.enqueue_nu_hook(crate::nu::NuHook::BufferOpen, buffer_open_args(path, kind));
	}

	/// Kicks one queued Nu hook evaluation onto the WorkScheduler.
	///
	/// Only kicks when no hook eval is already in flight (sequential
	/// evaluation preserves the single-threaded NuExecutor contract).
	/// Each kicked job receives a monotonic `job_id` for stale-result
	/// protection after runtime swaps.
	pub(crate) fn kick_nu_hook_eval(&mut self) {
		if self.state.nu_hook_in_flight.is_some() || self.state.nu_hook_queue.is_empty() {
			return;
		}

		let Some(queued) = self.state.nu_hook_queue.pop_front() else {
			return;
		};

		let fn_name = queued.hook.fn_name();
		let decl_id = match queued.hook {
			crate::nu::NuHook::ActionPost => self.state.nu_hook_ids.on_action_post,
			crate::nu::NuHook::CommandPost => self.state.nu_hook_ids.on_command_post,
			crate::nu::NuHook::EditorCommandPost => self.state.nu_hook_ids.on_editor_command_post,
			crate::nu::NuHook::ModeChange => self.state.nu_hook_ids.on_mode_change,
			crate::nu::NuHook::BufferOpen => self.state.nu_hook_ids.on_buffer_open,
		};

		let Some(decl_id) = decl_id else {
			return;
		};

		if self.ensure_nu_executor().is_none() {
			return;
		}

		let limits = self
			.state
			.config
			.nu
			.as_ref()
			.map_or_else(crate::nu::DecodeLimits::hook_defaults, |c| c.hook_decode_limits());
		let nu_ctx = self.build_nu_ctx("hook", fn_name, &queued.args);
		let env = vec![("XENO_CTX".to_string(), nu_ctx)];

		let executor_client = self.state.nu_executor.as_ref().unwrap().client();
		let msg_tx = self.state.msg_tx.clone();

		let job_id = self.state.nu_hook_job_next;
		self.state.nu_hook_job_next = self.state.nu_hook_job_next.wrapping_add(1);

		self.state.nu_hook_in_flight = Some(super::InFlightNuHook {
			job_id,
			hook: queued.hook,
			args: queued.args.clone(),
			retries: queued.retries,
		});

		let args_for_eval = queued.args;

		self.state.work_scheduler.schedule(crate::scheduler::WorkItem {
			future: Box::pin(async move {
				let result = match executor_client.run(decl_id, args_for_eval, limits, env).await {
					Ok(invocations) => Ok(invocations),
					Err(crate::nu::executor::NuExecError::Eval(msg)) => Err(crate::msg::NuHookEvalError::Eval(msg)),
					Err(crate::nu::executor::NuExecError::Shutdown { .. }) => Err(crate::msg::NuHookEvalError::ExecutorShutdown),
					Err(crate::nu::executor::NuExecError::ReplyDropped) => Err(crate::msg::NuHookEvalError::ReplyDropped),
				};
				let _ = msg_tx.send(crate::msg::EditorMsg::NuHookEvalDone(crate::msg::NuHookEvalDoneMsg { job_id, result }));
			}),
			kind: crate::scheduler::WorkKind::NuHook,
			priority: xeno_registry::hooks::HookPriority::Interactive,
			doc_id: None,
		});
	}

	/// Applies the result of an async Nu hook evaluation.
	///
	/// Ignores stale results (job_id mismatch after runtime swap).
	/// On executor death, restarts the executor and retries once.
	pub(crate) fn apply_nu_hook_eval_done(&mut self, msg: crate::msg::NuHookEvalDoneMsg) -> crate::msg::Dirty {
		let in_flight_job_id = self.state.nu_hook_in_flight.as_ref().map(|i| i.job_id);
		if in_flight_job_id != Some(msg.job_id) {
			// Stale result from a previous runtime — ignore.
			return crate::msg::Dirty::NONE;
		}

		let in_flight = self.state.nu_hook_in_flight.take().unwrap();

		match msg.result {
			Ok(invocations) => {
				let dirty = if invocations.is_empty() {
					crate::msg::Dirty::NONE
				} else {
					crate::msg::Dirty::FULL
				};
				self.state.nu_hook_pending_invocations.extend(invocations);
				dirty
			}
			Err(crate::msg::NuHookEvalError::Eval(error)) => {
				warn!(error = %error, "Nu hook evaluation failed");
				crate::msg::Dirty::NONE
			}
			Err(crate::msg::NuHookEvalError::ExecutorShutdown | crate::msg::NuHookEvalError::ReplyDropped) => {
				warn!("Nu executor died during hook eval, restarting");
				self.restart_nu_executor();
				if in_flight.retries == 0 {
					self.state.nu_hook_queue.push_front(super::QueuedNuHook {
						hook: in_flight.hook,
						args: in_flight.args,
						retries: 1,
					});
				} else {
					self.state.nu_hook_failed_total += 1;
					warn!(failed_total = self.state.nu_hook_failed_total, "Nu hook retry exhausted");
				}
				crate::msg::Dirty::NONE
			}
		}
	}

	/// Drains pending Nu hook invocations under the depth guard.
	///
	/// Called from pump() after message drain. Executes invocations produced
	/// by completed hook evaluations. Returns true if any produced quit.
	pub(crate) async fn drain_nu_hook_invocations(&mut self, max: usize) -> bool {
		if self.state.nu_hook_pending_invocations.is_empty() {
			return false;
		}

		self.state.nu_hook_depth += 1;

		for _ in 0..max {
			let Some(invocation) = self.state.nu_hook_pending_invocations.pop_front() else {
				break;
			};

			let result = Box::pin(self.run_invocation(invocation, InvocationPolicy::enforcing())).await;

			match result {
				InvocationResult::Ok => {}
				InvocationResult::Quit | InvocationResult::ForceQuit => {
					self.state.nu_hook_depth = self.state.nu_hook_depth.saturating_sub(1);
					return true;
				}
				InvocationResult::NotFound(target) => {
					warn!(target = %target, "Nu hook invocation not found");
				}
				InvocationResult::CapabilityDenied(cap) => {
					warn!(capability = ?cap, "Nu hook invocation denied by capability");
				}
				InvocationResult::ReadonlyDenied => {
					warn!("Nu hook invocation denied by readonly mode");
				}
				InvocationResult::CommandError(error) => {
					warn!(error = %error, "Nu hook invocation failed");
				}
			}
		}

		self.state.nu_hook_depth = self.state.nu_hook_depth.saturating_sub(1);
		false
	}

	/// Legacy synchronous drain for tests that need immediate hook evaluation.
	///
	/// Evaluates hooks synchronously via the executor (blocks on each one).
	/// Only used in tests; production code uses kick + poll via pump().
	#[cfg(test)]
	pub(crate) async fn drain_nu_hook_queue(&mut self, max: usize) -> bool {
		if self.state.nu_hook_queue.is_empty() {
			return false;
		}

		let to_drain = max.min(self.state.nu_hook_queue.len());
		self.state.nu_hook_depth += 1;

		for _ in 0..to_drain {
			let Some(queued) = self.state.nu_hook_queue.pop_front() else {
				break;
			};

			match self.run_single_nu_hook_sync(queued.hook, queued.args).await {
				Some(InvocationResult::Quit) | Some(InvocationResult::ForceQuit) => {
					self.state.nu_hook_depth = self.state.nu_hook_depth.saturating_sub(1);
					return true;
				}
				_ => {}
			}
		}

		self.state.nu_hook_depth = self.state.nu_hook_depth.saturating_sub(1);
		false
	}

	/// Synchronous single-hook evaluation for tests.
	#[cfg(test)]
	async fn run_single_nu_hook_sync(&mut self, hook: crate::nu::NuHook, args: Vec<String>) -> Option<InvocationResult> {
		let fn_name = hook.fn_name();
		let decl_id = match hook {
			crate::nu::NuHook::ActionPost => self.state.nu_hook_ids.on_action_post,
			crate::nu::NuHook::CommandPost => self.state.nu_hook_ids.on_command_post,
			crate::nu::NuHook::EditorCommandPost => self.state.nu_hook_ids.on_editor_command_post,
			crate::nu::NuHook::ModeChange => self.state.nu_hook_ids.on_mode_change,
			crate::nu::NuHook::BufferOpen => self.state.nu_hook_ids.on_buffer_open,
		}?;

		if self.ensure_nu_executor().is_none() {
			return None;
		}

		let limits = self
			.state
			.config
			.nu
			.as_ref()
			.map_or_else(crate::nu::DecodeLimits::hook_defaults, |c| c.hook_decode_limits());
		let nu_ctx = self.build_nu_ctx("hook", fn_name, &args);

		let invocations = match self.run_nu_with_restart("hook", fn_name, decl_id, args, limits, nu_ctx).await {
			Ok(invocations) => invocations,
			Err(error) => {
				warn!(hook = fn_name, error = ?error, "Nu hook failed");
				return None;
			}
		};

		for invocation in invocations {
			let result = Box::pin(self.run_invocation(invocation, InvocationPolicy::enforcing())).await;

			match result {
				InvocationResult::Ok => {}
				InvocationResult::Quit | InvocationResult::ForceQuit => return Some(result),
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
		let nu_ctx = self.build_nu_ctx("macro", &fn_name, &args);

		let invocations = match self.run_nu_with_restart("macro", &fn_name, decl_id, args, limits, nu_ctx).await {
			Ok(invocations) => invocations,
			Err(error) => {
				let msg = exec_error_message(&error);
				self.show_notification(xeno_registry::notifications::keys::command_error(&msg));
				return InvocationResult::CommandError(msg);
			}
		};

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

	/// Run a Nu function on the executor with automatic restart-and-retry on shutdown.
	///
	/// Handles `Shutdown` (retry once after restart), `ReplyDropped` (restart,
	/// no retry), and `Eval` errors uniformly.
	async fn run_nu_with_restart(
		&mut self,
		label: &str,
		fn_name: &str,
		decl_id: nu_protocol::DeclId,
		args: Vec<String>,
		limits: crate::nu::DecodeLimits,
		nu_ctx: Value,
	) -> Result<Vec<Invocation>, crate::nu::executor::NuExecError> {
		let start = Instant::now();
		let env = vec![("XENO_CTX".to_string(), nu_ctx)];

		let invocations = match self.state.nu_executor.as_ref().unwrap().run(decl_id, args, limits, env).await {
			Ok(invocations) => invocations,
			Err(crate::nu::executor::NuExecError::Shutdown { decl_id, args, limits, env }) => {
				warn!(%label, function = fn_name, "Nu executor died, restarting");
				self.restart_nu_executor();
				match self.ensure_nu_executor() {
					Some(executor) => executor.run(decl_id, args, limits, env).await?,
					None => {
						return Err(crate::nu::executor::NuExecError::Eval("Nu executor could not be restarted".to_string()));
					}
				}
			}
			Err(crate::nu::executor::NuExecError::ReplyDropped) => {
				warn!(%label, function = fn_name, "Nu executor worker died mid-evaluation, restarting");
				self.restart_nu_executor();
				return Err(crate::nu::executor::NuExecError::ReplyDropped);
			}
			Err(e) => return Err(e),
		};

		let elapsed = start.elapsed();
		let threshold = if label == "hook" { SLOW_NU_HOOK_THRESHOLD } else { SLOW_NU_MACRO_THRESHOLD };
		if elapsed > threshold {
			debug!(%label, function = fn_name, elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}

		Ok(invocations)
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
			&mut self.state.work_scheduler,
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
			&mut self.state.work_scheduler,
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
