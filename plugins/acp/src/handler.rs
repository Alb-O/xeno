use std::path::{Path, PathBuf};

use agent_client_protocol::{
	AgentNotification, AgentRequest, ClientResponse, ClientSide, ContentBlock, MessageHandler,
	ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse,
	SelectedPermissionOutcome, SessionUpdate, WriteTextFileResponse,
};
use tome_cabi_types::{TomeChatRole, TomeOwnedStr, TomeStatus, TomeStr};

use crate::events::{enqueue_panel_append, request_permission};
use crate::state::{HostHandle, SharedState};

pub struct PluginMessageHandler {
	pub host: HostHandle,
	pub state: SharedState,
}

impl MessageHandler<ClientSide> for PluginMessageHandler {
	#[allow(clippy::manual_async_fn)]
	fn handle_request(
		&self,
		request: AgentRequest,
	) -> impl std::future::Future<Output = agent_client_protocol::Result<ClientResponse>> {
		let state = self.state.clone();
		let host_ptr = self.host.host;

		async move {
			match request {
				AgentRequest::ReadTextFileRequest(req) => {
					handle_read_file(req, &state, host_ptr.0).await
				}
				AgentRequest::WriteTextFileRequest(req) => {
					handle_write_file(req, &state, host_ptr.0).await
				}
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

async fn handle_read_file(
	req: agent_client_protocol::ReadTextFileRequest,
	state: &SharedState,
	host_ptr: *const tome_cabi_types::TomeHostV2,
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

	let host = unsafe { &*host_ptr };
	if let Some(fs_read) = host.fs_read_text {
		let mut owned = unsafe { std::mem::zeroed::<TomeOwnedStr>() };
		let path_lossy = req.path.to_string_lossy();
		let ts = TomeStr {
			ptr: path_lossy.as_ptr(),
			len: path_lossy.len(),
		};
		if fs_read(ts, &mut owned) == TomeStatus::Ok {
			let content = crate::utils::tome_owned_to_string(owned).unwrap_or_default();
			if let Some(free_str) = host.free_str {
				free_str(owned);
			}
			return Ok(ClientResponse::ReadTextFileResponse(
				ReadTextFileResponse::new(content),
			));
		}
	}

	let content = std::fs::read_to_string(&req.path)
		.map_err(|e| agent_client_protocol::Error::new(-32000, e.to_string()))?;
	Ok(ClientResponse::ReadTextFileResponse(
		ReadTextFileResponse::new(content),
	))
}

async fn handle_write_file(
	req: agent_client_protocol::WriteTextFileRequest,
	state: &SharedState,
	host_ptr: *const tome_cabi_types::TomeHostV2,
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

	let host = unsafe { &*host_ptr };
	if let Some(fs_write) = host.fs_write_text {
		let path_lossy = req.path.to_string_lossy();
		let ts_path = TomeStr {
			ptr: path_lossy.as_ptr(),
			len: path_lossy.len(),
		};
		let ts_content = TomeStr {
			ptr: req.content.as_ptr(),
			len: req.content.len(),
		};
		if fs_write(ts_path, ts_content) == TomeStatus::Ok {
			return Ok(ClientResponse::WriteTextFileResponse(
				WriteTextFileResponse::new(),
			));
		}
	}

	std::fs::write(&req.path, &req.content)
		.map_err(|e| agent_client_protocol::Error::new(-32000, e.to_string()))?;
	Ok(ClientResponse::WriteTextFileResponse(
		WriteTextFileResponse::new(),
	))
}

async fn handle_permission_request(
	req: agent_client_protocol::RequestPermissionRequest,
	state: &SharedState,
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

fn handle_session_update(update: SessionUpdate, state: &SharedState) {
	let pid = match *state.panel_id.lock() {
		Some(id) => id,
		None => return,
	};

	match update {
		SessionUpdate::AgentMessageChunk(chunk) => {
			if let ContentBlock::Text(text) = chunk.content {
				{
					let mut last = state.last_assistant_text.lock();
					last.push_str(&text.text);
				}
				enqueue_panel_append(&state.events, pid, TomeChatRole::Assistant, text.text);
			}
		}
		SessionUpdate::AgentThoughtChunk(chunk) => {
			if let ContentBlock::Text(text) = chunk.content {
				enqueue_panel_append(&state.events, pid, TomeChatRole::Thought, text.text);
			}
		}
		_ => {}
	}
}

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
