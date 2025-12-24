//! ACP types shared across the application.
//!
//! This module contains the core types for ACP functionality that are used
//! by both the ACP backend and the UI layer. By centralizing these types,
//! we avoid scattered imports and maintain a clean dependency graph.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use parking_lot::Mutex;
use tokio::sync::oneshot;
use tome_core::Rope;

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

/// A single item in the chat transcript.
#[derive(Debug, Clone)]
pub struct ChatItem {
	pub role: ChatRole,
	pub text: String,
}

/// State for a chat panel.
pub struct ChatPanelState {
	pub transcript: Vec<ChatItem>,
	pub input: Rope,
	pub input_cursor: usize,
}

impl ChatPanelState {
	pub fn new(_title: String) -> Self {
		Self {
			transcript: Vec::new(),
			input: Rope::from(""),
			input_cursor: 0,
		}
	}
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

/// Permission option for user decisions.
#[derive(Debug, Clone)]
pub struct PermissionOption {
	#[allow(dead_code, reason = "Required for interactive permission dialogs")]
	pub id: String,
	#[allow(dead_code, reason = "Required for interactive permission dialogs")]
	pub label: String,
}

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
	/// Panels managed by ACP.
	pub panels: Arc<Mutex<HashMap<u64, ChatPanelState>>>,
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
			panels: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	/// Generate a unique permission request ID.
	pub fn next_permission_id(&self) -> u64 {
		use std::sync::atomic::Ordering;
		self.next_permission_id.fetch_add(1, Ordering::SeqCst)
	}
}

impl Default for AcpState {
	fn default() -> Self {
		Self::new()
	}
}
