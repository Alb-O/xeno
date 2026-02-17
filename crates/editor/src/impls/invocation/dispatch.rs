use std::collections::VecDeque;

use tracing::{trace, trace_span};
use xeno_invocation::{CommandInvocation, CommandRoute};

use super::hooks_bridge::{action_post_args, command_post_args};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy, InvocationStatus, InvocationTarget};

const MAX_NU_MACRO_DEPTH: u8 = 8;

#[derive(Debug)]
struct QueuedInvocation {
	invocation: Invocation,
	nu_depth: u8,
}

impl Editor {
	/// Executes a named action with enforcement defaults.
	pub fn invoke_action(&mut self, name: &str, count: usize, extend: bool, register: Option<char>, char_arg: Option<char>) -> InvocationOutcome {
		self.run_action_invocation(name, count, extend, register, char_arg, InvocationPolicy::enforcing())
	}

	/// Executes a command invocation with enforcement defaults.
	pub async fn invoke_command(&mut self, name: &str, args: Vec<String>) -> InvocationOutcome {
		self.run_command_invocation(name, &args, CommandRoute::Auto, InvocationPolicy::enforcing())
			.await
	}

	/// Executes one invocation root and drains follow-up dispatches iteratively.
	///
	/// Unified entry point for keymap dispatch, command palette, ex commands,
	/// and hook-triggered invocations.
	pub async fn run_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationOutcome {
		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		let mut queue = VecDeque::from([QueuedInvocation { invocation, nu_depth: 0 }]);
		let mut last_outcome = InvocationOutcome::ok(InvocationTarget::Command);

		while let Some(queued) = queue.pop_front() {
			let (outcome, follow_ups) = self.run_single_invocation_step(queued.invocation, queued.nu_depth, policy).await;
			last_outcome = outcome.clone();

			if !outcome.is_quit() && !follow_ups.is_empty() {
				for queued in follow_ups.into_iter().rev() {
					queue.push_front(queued);
				}
			}

			if !matches!(outcome.status, InvocationStatus::Ok) {
				return outcome;
			}
		}

		last_outcome
	}

	async fn run_single_invocation_step(
		&mut self,
		invocation: Invocation,
		nu_depth: u8,
		policy: InvocationPolicy,
	) -> (InvocationOutcome, Vec<QueuedInvocation>) {
		match invocation {
			Invocation::Action { name, count, extend, register } => {
				let outcome = self.run_action_invocation(&name, count, extend, register, None, policy);
				if !outcome.is_quit() {
					self.enqueue_nu_hook(crate::nu::NuHook::ActionPost, action_post_args(name, &outcome));
				}
				(outcome, Vec::new())
			}
			Invocation::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => {
				let outcome = self.run_action_invocation(&name, count, extend, register, Some(char_arg), policy);
				if !outcome.is_quit() {
					self.enqueue_nu_hook(crate::nu::NuHook::ActionPost, action_post_args(name, &outcome));
				}
				(outcome, Vec::new())
			}
			Invocation::Command(CommandInvocation { name, args, route }) => {
				let (outcome, resolved_route) = self.run_command_invocation_with_resolved_route(&name, &args, route, policy).await;
				if !outcome.is_quit() {
					let hook = if resolved_route == CommandRoute::Editor {
						crate::nu::NuHook::EditorCommandPost
					} else {
						crate::nu::NuHook::CommandPost
					};
					self.enqueue_nu_hook(hook, command_post_args(name, &outcome, args));
				}
				(outcome, Vec::new())
			}
			Invocation::Nu { name, args } => {
				if nu_depth >= MAX_NU_MACRO_DEPTH {
					return (
						InvocationOutcome::command_error(InvocationTarget::Nu, format!("Nu macro recursion depth exceeded ({MAX_NU_MACRO_DEPTH})")),
						Vec::new(),
					);
				}

				self.state.nu.inc_macro_depth();
				let result = self.run_nu_macro_invocation(name, args).await;
				self.state.nu.dec_macro_depth();

				match result {
					Ok(follow_ups) => (
						InvocationOutcome::ok(InvocationTarget::Nu),
						follow_ups
							.into_iter()
							.map(|invocation| QueuedInvocation {
								invocation,
								nu_depth: nu_depth.saturating_add(1),
							})
							.collect(),
					),
					Err(outcome) => (outcome, Vec::new()),
				}
			}
		}
	}
}
