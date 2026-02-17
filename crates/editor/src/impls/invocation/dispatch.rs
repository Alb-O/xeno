use tracing::{trace, trace_span};
use xeno_invocation::CommandRoute;

use super::engine::InvocationEngine;
use crate::impls::Editor;
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy};

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

	/// Executes an invocation through the canonical queue-driven engine.
	pub async fn run_invocation(&mut self, invocation: Invocation, policy: InvocationPolicy) -> InvocationOutcome {
		let span = trace_span!("run_invocation", invocation = %invocation.describe());
		let _guard = span.enter();
		trace!(policy = ?policy, "Running invocation");

		InvocationEngine::new(self, policy).run(invocation).await
	}
}
