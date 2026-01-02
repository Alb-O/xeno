//! ACP message handler implementation.
//!
//! This module implements the MessageHandler trait from agent-client-protocol
//! to handle requests and notifications from the agent.

use std::path::{Path, PathBuf};

use agent_client_protocol::{
	AgentNotification, AgentRequest, ClientResponse, ClientSide, ContentBlock, MessageHandler,
	ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse,
	SelectedPermissionOutcome, SessionUpdate, WriteTextFileResponse,
};

use crate::backend::enqueue_line;
use crate::types::{AcpEvent, AcpState, PermissionOption};

/// Handler for ACP protocol messages.
pub struct AcpMessageHandler {
	/// Shared state for cross-thread communication.
	pub state: AcpState,
}

impl MessageHandler<ClientSide> for AcpMessageHandler {
	#[allow(
		clippy::manual_async_fn,
		reason = "trait requires non-async signature with impl Future return"
	)]
	fn handle_request(
		&self,
		request: AgentRequest,
	) -> impl std::future::Future<Output = agent_client_protocol::Result<ClientResponse>> {
		let state = self.state.clone();

		async move {
			match request {
				AgentRequest::ReadTextFileRequest(req) => handle_read_file(req, &state).await,
				AgentRequest::WriteTextFileRequest(req) => handle_write_file(req, &state).await,
				AgentRequest::RequestPermissionRequest(req) => {
					handle_permission_request(req, &state).await
				}
				_ => Err(agent_client_protocol::Error::method_not_found()),
			}
		}
	}

	fn handle_notification(
		&self,
		notification: AgentNotification,
	) -> impl std::future::Future<Output = agent_client_protocol::Result<()>> {
		let state = self.state.clone();
		async move {
			if let AgentNotification::SessionNotification(sn) = notification {
				handle_session_update(sn.update, &state);
			}
			Ok(())
		}
	}
}

/// Handles a file read request from the agent.
async fn handle_read_file(
	req: agent_client_protocol::ReadTextFileRequest,
	state: &AcpState,
) -> agent_client_protocol::Result<ClientResponse> {
	let path = PathBuf::from(&req.path);
	let root = state.workspace_root.lock().clone();
	if !is_path_in_workspace(&path, &root) {
		return Err(agent_client_protocol::Error::new(
			-32000,
			"Access denied: path outside workspace".to_string(),
		));
	}

	let prompt = format!("Allow reading file: {}", req.path.display());
	if !request_permission(state, &prompt).await? {
		return Err(agent_client_protocol::Error::new(
			-32000,
			"Permission denied by user".to_string(),
		));
	}

	let content = std::fs::read_to_string(&req.path)
		.map_err(|e| agent_client_protocol::Error::new(-32000, e.to_string()))?;
	Ok(ClientResponse::ReadTextFileResponse(
		ReadTextFileResponse::new(content),
	))
}

/// Handles a file write request from the agent.
async fn handle_write_file(
	req: agent_client_protocol::WriteTextFileRequest,
	state: &AcpState,
) -> agent_client_protocol::Result<ClientResponse> {
	let path = PathBuf::from(&req.path);
	let root = state.workspace_root.lock().clone();
	if !is_path_in_workspace(&path, &root) {
		return Err(agent_client_protocol::Error::new(
			-32000,
			"Access denied: path outside workspace".to_string(),
		));
	}

	let prompt = format!("Allow writing to file: {}", req.path.display());
	if !request_permission(state, &prompt).await? {
		return Err(agent_client_protocol::Error::new(
			-32000,
			"Permission denied by user".to_string(),
		));
	}

	std::fs::write(&req.path, &req.content)
		.map_err(|e| agent_client_protocol::Error::new(-32000, e.to_string()))?;
	Ok(ClientResponse::WriteTextFileResponse(
		WriteTextFileResponse::new(),
	))
}

/// Handles a permission request from the agent.
async fn handle_permission_request(
	req: agent_client_protocol::RequestPermissionRequest,
	state: &AcpState,
) -> agent_client_protocol::Result<ClientResponse> {
	let prompt = format!("Agent requested permission for session {}", req.session_id);
	if !req.options.is_empty() && request_permission(state, &prompt).await? {
		let outcome = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
			req.options[0].option_id.clone(),
		));
		Ok(ClientResponse::RequestPermissionResponse(
			RequestPermissionResponse::new(outcome),
		))
	} else {
		Err(agent_client_protocol::Error::new(
			-32000,
			"Permission denied by user or no options available".to_string(),
		))
	}
}

/// Processes a session update notification from the agent.
fn handle_session_update(update: SessionUpdate, state: &AcpState) {
	match update {
		SessionUpdate::AgentMessageChunk(chunk) => {
			if let ContentBlock::Text(text) = chunk.content {
				// Store assistant text for insert_last command
				{
					let mut last = state.last_assistant_text.lock();
					last.push_str(&text.text);
				}
				enqueue_line(state, format!("[Assistant] {}", text.text));
			}
		}
		SessionUpdate::AgentThoughtChunk(chunk) => {
			if let ContentBlock::Text(text) = chunk.content {
				enqueue_line(state, format!("[Thought] {}", text.text));
			}
		}
		_ => {}
	}
}

/// Checks if a path is within the workspace root.
fn is_path_in_workspace(path: &Path, root: &Option<PathBuf>) -> bool {
	let root = match root {
		Some(r) => r,
		None => return false,
	};

	let canon = path.canonicalize().or_else(|_| {
		path.parent()
			.and_then(|p| {
				p.canonicalize()
					.ok()
					.map(|cp| cp.join(path.file_name().unwrap_or_default()))
			})
			.ok_or(())
	});

	match canon {
		Ok(p) => p.starts_with(root),
		Err(_) => false,
	}
}

/// Request permission from the user.
///
/// This creates a permission request event and waits for the user's decision.
async fn request_permission(state: &AcpState, prompt: &str) -> agent_client_protocol::Result<bool> {
	let (tx, rx) = tokio::sync::oneshot::channel();
	let id = state.next_permission_id();

	{
		let mut pending = state.pending_permissions.lock();
		pending.insert(id, tx);
	}

	let options = vec![
		PermissionOption {
			id: "allow".to_string(),
			label: "Allow".to_string(),
		},
		PermissionOption {
			id: "deny".to_string(),
			label: "Deny".to_string(),
		},
	];

	{
		let mut events = state.events.lock();
		events.push(AcpEvent::RequestPermission {
			id,
			prompt: prompt.to_string(),
			options,
		});
	}

	match rx.await {
		Ok(decision) => Ok(decision == "allow"),
		Err(_) => Err(agent_client_protocol::Error::new(
			-32603,
			"Internal error: permission channel closed".to_string(),
		)),
	}
}
