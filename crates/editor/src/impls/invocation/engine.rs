use std::collections::VecDeque;

use tracing::trace;
use xeno_invocation::{CommandInvocation, CommandRoute};

use super::hooks_bridge::{action_post_event, command_post_event, editor_command_post_event};
use crate::impls::Editor;
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy, InvocationStatus, InvocationTarget};

const MAX_NU_MACRO_DEPTH: u8 = 8;

#[derive(Debug, Clone, Copy)]
enum InvocationOrigin {
	Root,
	NuMacro,
}

#[derive(Debug)]
pub(super) struct InvocationFrame {
	invocation: Invocation,
	nu_depth: u8,
	origin: InvocationOrigin,
}

impl InvocationFrame {
	fn root(invocation: Invocation) -> Self {
		Self {
			invocation,
			nu_depth: 0,
			origin: InvocationOrigin::Root,
		}
	}

	fn nu_follow_up(invocation: Invocation, nu_depth: u8) -> Self {
		Self {
			invocation,
			nu_depth,
			origin: InvocationOrigin::NuMacro,
		}
	}
}

#[derive(Debug)]
enum InvocationPostHook {
	Action { name: String },
	Command { name: String, args: Vec<String>, is_editor: bool },
}

#[derive(Debug)]
pub(super) struct InvocationStepOutcome {
	outcome: InvocationOutcome,
	follow_ups: Vec<InvocationFrame>,
	post_hook: Option<InvocationPostHook>,
}

pub(super) struct InvocationEngine<'a> {
	editor: &'a mut Editor,
	policy: InvocationPolicy,
	queue: VecDeque<InvocationFrame>,
}

impl<'a> InvocationEngine<'a> {
	pub(super) fn new(editor: &'a mut Editor, policy: InvocationPolicy) -> Self {
		Self {
			editor,
			policy,
			queue: VecDeque::new(),
		}
	}

	pub(super) async fn run(mut self, root: Invocation) -> InvocationOutcome {
		self.queue.push_back(InvocationFrame::root(root));
		let mut last_outcome = InvocationOutcome::ok(InvocationTarget::Command);

		while let Some(frame) = self.queue.pop_front() {
			trace!(origin = ?frame.origin, nu_depth = frame.nu_depth, invocation = %frame.invocation.describe(), "invocation.engine.frame");
			let step = self.run_frame(frame).await;
			last_outcome = step.outcome.clone();

			if let Some(post_hook) = step.post_hook {
				self.apply_post_hook(post_hook, &step.outcome);
			}

			if !matches!(step.outcome.status, InvocationStatus::Ok) {
				return step.outcome;
			}

			self.enqueue_follow_ups(step.follow_ups);
		}

		last_outcome
	}

	fn enqueue_follow_ups(&mut self, follow_ups: Vec<InvocationFrame>) {
		for frame in follow_ups.into_iter().rev() {
			self.queue.push_front(frame);
		}
	}

	fn apply_post_hook(&mut self, hook: InvocationPostHook, outcome: &InvocationOutcome) {
		if outcome.is_quit() {
			return;
		}

		match hook {
			InvocationPostHook::Action { name } => {
				self.editor.enqueue_nu_hook(action_post_event(name, outcome));
			}
			InvocationPostHook::Command { name, args, is_editor } => {
				let event = if is_editor {
					editor_command_post_event(name, outcome, args)
				} else {
					command_post_event(name, outcome, args)
				};
				self.editor.enqueue_nu_hook(event);
			}
		}
	}

	async fn run_frame(&mut self, frame: InvocationFrame) -> InvocationStepOutcome {
		match frame.invocation {
			Invocation::Action { name, count, extend, register } => {
				let outcome = self.editor.run_action_invocation(&name, count, extend, register, None, self.policy);
				InvocationStepOutcome {
					outcome,
					follow_ups: Vec::new(),
					post_hook: Some(InvocationPostHook::Action { name }),
				}
			}
			Invocation::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => {
				let outcome = self.editor.run_action_invocation(&name, count, extend, register, Some(char_arg), self.policy);
				InvocationStepOutcome {
					outcome,
					follow_ups: Vec::new(),
					post_hook: Some(InvocationPostHook::Action { name }),
				}
			}
			Invocation::Command(CommandInvocation { name, args, route }) => {
				let (outcome, resolved_route) = self.editor.run_command_invocation_with_resolved_route(&name, &args, route, self.policy).await;
				InvocationStepOutcome {
					outcome,
					follow_ups: Vec::new(),
					post_hook: Some(InvocationPostHook::Command {
						name,
						args,
						is_editor: resolved_route == CommandRoute::Editor,
					}),
				}
			}
			Invocation::Nu { name, args } => {
				if frame.nu_depth >= MAX_NU_MACRO_DEPTH {
					return InvocationStepOutcome {
						outcome: InvocationOutcome::command_error(InvocationTarget::Nu, format!("Nu macro recursion depth exceeded ({MAX_NU_MACRO_DEPTH})")),
						follow_ups: Vec::new(),
						post_hook: None,
					};
				}

				self.editor.state.nu.inc_macro_depth();
				let result = self.editor.run_nu_macro_invocation(name, args).await;
				self.editor.state.nu.dec_macro_depth();

				match result {
					Ok(follow_ups) => InvocationStepOutcome {
						outcome: InvocationOutcome::ok(InvocationTarget::Nu),
						follow_ups: follow_ups
							.into_iter()
							.map(|invocation| InvocationFrame::nu_follow_up(invocation, frame.nu_depth.saturating_add(1)))
							.collect(),
						post_hook: None,
					},
					Err(outcome) => InvocationStepOutcome {
						outcome,
						follow_ups: Vec::new(),
						post_hook: None,
					},
				}
			}
		}
	}
}
