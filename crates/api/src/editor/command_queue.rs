//! Command queue for deferred command execution.
//!
//! When an action returns [`ActionResult::Command`], the command is queued here
//! for async execution on the next tick.

use std::collections::VecDeque;

/// A queued command to be executed asynchronously.
#[derive(Debug, Clone)]
pub struct QueuedCommand {
	/// Command name.
	pub name: &'static str,
	/// Command arguments.
	pub args: Vec<String>,
}

/// Queue for commands to be executed asynchronously.
///
/// Actions can schedule commands via [`ActionResult::Command`]. The main loop
/// drains this queue and executes commands with full async/editor access.
#[derive(Default)]
pub struct CommandQueue {
	/// Pending commands awaiting execution.
	queue: VecDeque<QueuedCommand>,
}

impl CommandQueue {
	/// Creates an empty command queue.
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a command to the queue.
	pub fn push(&mut self, name: &'static str, args: Vec<String>) {
		self.queue.push_back(QueuedCommand { name, args });
	}

	/// Drains all pending commands from the queue.
	pub fn drain(&mut self) -> impl Iterator<Item = QueuedCommand> + '_ {
		self.queue.drain(..)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn queue_push_and_drain() {
		let mut queue = CommandQueue::new();

		queue.push("lsp-hover", vec![]);
		queue.push("lsp-goto-definition", vec!["--include-declaration".into()]);

		let commands: Vec<_> = queue.drain().collect();
		assert_eq!(commands.len(), 2);
		assert_eq!(commands[0].name, "lsp-hover");
		assert_eq!(commands[1].name, "lsp-goto-definition");
	}
}
