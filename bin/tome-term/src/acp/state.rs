//! ACP manager - the main entry point for ACP functionality.
//!
//! This module contains only the AcpManager which orchestrates the backend.
//! All shared types are defined in the `types` module.

use std::path::PathBuf;
use std::thread;

use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Sender};
use tokio::task::LocalSet;

use crate::acp::backend::AcpBackend;
use crate::acp::types::{AcpEvent, AcpState, AgentCommand};

/// Manager for the ACP integration.
///
/// This is the main entry point for the ACP functionality. It manages the
/// background thread running the agent communication and provides methods
/// for sending commands and receiving events.
pub struct AcpManager {
	/// Channel for sending commands to the backend.
	cmd_tx: Option<Sender<AgentCommand>>,
	/// Shared state accessible from UI.
	pub state: AcpState,
	/// Whether the agent has been started.
	started: bool,
}

impl AcpManager {
	pub fn new() -> Self {
		Self {
			cmd_tx: None,
			state: AcpState::new(),
			started: false,
		}
	}

	/// Start the ACP backend thread.
	///
	/// This spawns a background thread with a tokio runtime that handles
	/// all async communication with the agent.
	pub fn start(&mut self, cwd: PathBuf) {
		if self.started {
			// Already started - just send the start command.
			if let Some(tx) = &self.cmd_tx {
				let _ = tx.try_send(AgentCommand::Start { cwd });
			}
			return;
		}

		let (cmd_tx, cmd_rx) = mpsc::channel(100);
		self.cmd_tx = Some(cmd_tx.clone());
		self.started = true;

		let state = self.state.clone();

		thread::spawn(move || {
			let rt = Runtime::new().unwrap();
			let local = LocalSet::new();

			local.block_on(&rt, async {
				AcpBackend::new(cmd_rx, state.clone()).run(cwd).await;
			});
		});
	}

	/// Stop the ACP backend.
	pub fn stop(&mut self) {
		if let Some(tx) = &self.cmd_tx {
			let _ = tx.try_send(AgentCommand::Stop);
		}
		self.started = false;
	}

	/// Send a prompt to the agent.
	pub fn prompt(&self, content: String) {
		if let Some(tx) = &self.cmd_tx {
			let _ = tx.try_send(AgentCommand::Prompt { content });
		}
	}

	/// Cancel the current in-flight request.
	pub fn cancel(&self) {
		if let Some(tx) = &self.cmd_tx {
			let _ = tx.try_send(AgentCommand::Cancel);
		}
	}

	/// Check if the agent has been started.
	#[allow(dead_code, reason = "Public API for external status checks")]
	pub fn is_started(&self) -> bool {
		self.started
	}

	/// Get the last assistant response text.
	pub fn last_assistant_text(&self) -> String {
		self.state.last_assistant_text.lock().clone()
	}

	/// Drain pending events from the queue.
	pub fn drain_events(&self) -> Vec<AcpEvent> {
		let mut events = self.state.events.lock();
		std::mem::take(&mut *events)
	}

	/// Set the panel ID.
	pub fn set_panel_id(&self, id: Option<u64>) {
		*self.state.panel_id.lock() = id;
	}

	/// Get the current panel ID.
	pub fn panel_id(&self) -> Option<u64> {
		*self.state.panel_id.lock()
	}

	/// Handle a permission decision from the user.
	pub fn on_permission_decision(&self, request_id: u64, option_id: String) {
		let mut pending = self.state.pending_permissions.lock();
		if let Some(tx) = pending.remove(&request_id) {
			let _ = tx.send(option_id);
		}
	}
}

impl Default for AcpManager {
	fn default() -> Self {
		Self::new()
	}
}
