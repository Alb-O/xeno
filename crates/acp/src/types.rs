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

/// Chat message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
	/// Message from the user.
	#[allow(
		dead_code,
		reason = "Required for full ACP spec compliance and future UI support"
	)]
	User,
	/// Response from the AI assistant.
	Assistant,
	/// System prompt or instructions.
	System,
	/// Internal reasoning from the AI (chain of thought).
	Thought,
}

/// Events produced by the ACP backend for the UI to consume.
#[derive(Debug)]
pub enum AcpEvent {
	/// Show a message notification.
	ShowMessage(String),
	/// Request permission from the user.
	RequestPermission {
		/// Unique ID for this permission request.
		id: u64,
		/// Human-readable description of what permission is being requested.
		prompt: String,
		/// Available response options for the user.
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
	/// Unique identifier for this option.
	#[allow(dead_code, reason = "Required for interactive permission dialogs")]
	pub id: String,
	/// Human-readable label for this option.
	#[allow(dead_code, reason = "Required for interactive permission dialogs")]
	pub label: String,
}

/// Commands that can be sent to the ACP backend.
#[derive(Debug)]
pub enum AgentCommand {
	/// Start the agent in the specified working directory.
	Start {
		/// Working directory for the agent session.
		cwd: PathBuf,
	},
	/// Stop the agent.
	Stop,
	/// Send a prompt to the agent.
	Prompt {
		/// The prompt text to send to the agent.
		content: String,
	},
	/// Cancel the current in-flight request.
	Cancel,
	/// Set the model for the current session.
	SetModel {
		/// The model identifier to use (e.g., "anthropic/claude-sonnet-4").
		model_id: String,
	},
}

/// Re-export ModelInfo from agent-client-protocol.
pub use agent_client_protocol::ModelInfo;

/// Default model to use if none is configured.
pub const DEFAULT_MODEL: &str = "opencode/big-pickle";

/// Shared state accessible from multiple threads.
#[derive(Clone)]
pub struct AcpState {
	/// Event queue for UI consumption.
	pub events: Arc<Mutex<Vec<AcpEvent>>>,
	/// Last assistant response text (for insert_last command).
	pub last_assistant_text: Arc<Mutex<String>>,
	/// Pending permission requests waiting for user decision.
	pub pending_permissions: Arc<Mutex<HashMap<u64, oneshot::Sender<String>>>>,
	/// Counter for generating unique permission request IDs.
	pub next_permission_id: Arc<AtomicU64>,
	/// Workspace root directory for security checks.
	pub workspace_root: Arc<Mutex<Option<PathBuf>>>,
	/// Current model ID (e.g., "opencode/big-pickle", "anthropic/claude-sonnet-4").
	pub current_model: Arc<Mutex<String>>,
	/// Available models from the agent (populated after session creation).
	pub available_models: Arc<Mutex<Vec<ModelInfo>>>,
}

impl AcpState {
	/// Creates a new ACP state with default settings.
	pub fn new() -> Self {
		Self {
			events: Arc::new(Mutex::new(Vec::new())),
			last_assistant_text: Arc::new(Mutex::new(String::new())),
			pending_permissions: Arc::new(Mutex::new(HashMap::new())),
			next_permission_id: Arc::new(AtomicU64::new(1)),
			workspace_root: Arc::new(Mutex::new(None)),
			current_model: Arc::new(Mutex::new(DEFAULT_MODEL.to_string())),
			available_models: Arc::new(Mutex::new(Vec::new())),
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
