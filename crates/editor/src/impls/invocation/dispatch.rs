use tracing::{trace, trace_span};

use super::hooks_bridge::{action_post_args, command_post_args};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

const MAX_NU_MACRO_DEPTH: u8 = 8;

impl Editor {
	/// Executes a named action with enforcement defaults.
	pub fn invoke_action(&mut self, name: &str, count: usize, extend: bool, register: Option<char>, char_arg: Option<char>) -> InvocationResult {
		self.run_action_invocation(name, count, extend, register, char_arg, InvocationPolicy::enforcing())
	}

	/// Executes a registry command with enforcement defaults.
	pub async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationResult {
		self.run_command_invocation(name, &args, InvocationPolicy::enforcing()).await
	}

	/// Executes an invocation with shared preflight policy gates and hook emission.
	///
	/// Unified entry point for keymap dispatch, command palette, ex commands,
	/// and hook-triggered invocations.
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
				if self.state.nu.macro_depth() >= MAX_NU_MACRO_DEPTH {
					return InvocationResult::CommandError(format!("Nu macro recursion depth exceeded ({MAX_NU_MACRO_DEPTH})"));
				}

				self.state.nu.inc_macro_depth();
				let result = self.run_nu_macro_invocation(name, args, policy).await;
				self.state.nu.dec_macro_depth();
				result
			}
		}
	}
}
