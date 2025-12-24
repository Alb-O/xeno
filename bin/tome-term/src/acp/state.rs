//! ACP state management.
//!
//! This module contains the shared state for the ACP integration, including
//! the event queue, panel state, and permission handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use parking_lot::Mutex;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;
use tokio::task::LocalSet;

use crate::acp::backend::AcpBackend;

/// Commands that can be sent to the ACP backend.
#[derive(Debug)]
pub enum AgentCommand {
	/// Start the agent in the specified working directory.
	Start { cwd: PathBuf },
	/// Stop the agent.
	Stop,
	/// Send a prompt to the agent.
	Prompt { content: String },
	/// Cancel the current in-flight request.
	Cancel,
}

/// Events produced by the ACP backend for the UI to consume.
#[derive(Debug)]
pub enum AcpEvent {
	/// Append a message to the chat panel.
	PanelAppend { role: ChatRole, text: String },
	/// Show a message (when no panel is open).
	ShowMessage(String),
	/// Request permission from the user.
	RequestPermission {
		id: u64,
		prompt: String,
		#[allow(
			dead_code,
			reason = "UI currently auto-allows; options will be used for interactive dialogs"
		)]
		options: Vec<PermissionOption>,
	},
}

/// Chat message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
	#[allow(
		dead_code,
		reason = "Required for full ACP spec compliance and future UI support"
	)]
	User,
	Assistant,
	System,
	Thought,
}

/// Permission option for user decisions.
#[derive(Debug, Clone)]
pub struct PermissionOption {
	#[allow(dead_code, reason = "Required for interactive permission dialogs")]
	pub id: String,
	#[allow(dead_code, reason = "Required for interactive permission dialogs")]
	pub label: String,
}

/// Shared state accessible from multiple threads.
#[derive(Clone)]
pub struct AcpState {
	/// Event queue for UI consumption.
	pub events: Arc<Mutex<Vec<AcpEvent>>>,
	/// Current panel ID (if panel is open).
	pub panel_id: Arc<Mutex<Option<u64>>>,
	/// Last assistant response text (for insert_last command).
	pub last_assistant_text: Arc<Mutex<String>>,
	/// Pending permission requests waiting for user decision.
	pub pending_permissions: Arc<Mutex<HashMap<u64, oneshot::Sender<String>>>>,
	/// Counter for generating unique permission request IDs.
	pub next_permission_id: Arc<AtomicU64>,
	/// Workspace root directory for security checks.
	pub workspace_root: Arc<Mutex<Option<PathBuf>>>,
}

impl AcpState {
	pub fn new() -> Self {
		Self {
			events: Arc::new(Mutex::new(Vec::new())),
			panel_id: Arc::new(Mutex::new(None)),
			last_assistant_text: Arc::new(Mutex::new(String::new())),
			pending_permissions: Arc::new(Mutex::new(HashMap::new())),
			next_permission_id: Arc::new(AtomicU64::new(1)),
			workspace_root: Arc::new(Mutex::new(None)),
		}
	}

	/// Generate a unique permission request ID.
	pub fn next_permission_id(&self) -> u64 {
		self.next_permission_id.fetch_add(1, Ordering::SeqCst)
	}
}

impl Default for AcpState {
	fn default() -> Self {
		Self::new()
	}
}

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
