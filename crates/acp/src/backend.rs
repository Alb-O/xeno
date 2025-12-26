//! ACP backend for async agent communication.
//!
//! This module handles the async communication with the ACP agent, running
//! in a dedicated thread with a tokio runtime.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::{
	Agent, CancelNotification, ClientCapabilities, ClientSideConnection, ContentBlock,
	FileSystemCapability, Implementation, InitializeRequest, NewSessionRequest, PromptRequest,
	ProtocolVersion, SetSessionModelRequest, TextContent,
};
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Receiver;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::handler::AcpMessageHandler;
use crate::types::{AcpEvent, AcpState, AgentCommand};

/// Backend for ACP agent communication.
pub struct AcpBackend {
	cmd_rx: Receiver<AgentCommand>,
	state: AcpState,
	session_id: Option<String>,
}

impl AcpBackend {
	pub fn new(cmd_rx: Receiver<AgentCommand>, state: AcpState) -> Self {
		Self {
			cmd_rx,
			state,
			session_id: None,
		}
	}

	/// Run the backend, processing commands until stopped.
	pub async fn run(mut self, initial_cwd: PathBuf) {
		{
			let mut root = self.state.workspace_root.lock();
			*root = Some(initial_cwd.clone());
		}

		if let Err(e) = self.start_agent(initial_cwd).await {
			let msg = format!("Failed to start agent: {e:?}");
			self.enqueue_message(msg);
		}
	}

	async fn start_agent(&mut self, cwd: PathBuf) -> agent_client_protocol::Result<()> {
		let mut child = Command::new("opencode")
			.arg("acp")
			.arg("--port")
			.arg("0")
			.current_dir(&cwd)
			.stdin(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.spawn()
			.map_err(|e| agent_client_protocol::Error::new(-32603, e.to_string()))?;

		let stdin = child.stdin.take().unwrap();
		let stdout = child.stdout.take().unwrap();
		let stderr = child.stderr.take();

		// Collect stderr for debugging purposes
		let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
		if let Some(stderr) = stderr {
			let state = self.state.clone();
			let stderr_tail = stderr_tail.clone();
			tokio::task::spawn_local(async move {
				let mut lines = BufReader::new(stderr).lines();
				while let Ok(Some(line)) = lines.next_line().await {
					let line = strip_ansi_and_controls(&line);
					if line.trim().is_empty() {
						continue;
					}

					{
						let mut tail = stderr_tail.lock();
						if tail.len() >= 50 {
							tail.pop_front();
						}
						tail.push_back(line.clone());
					}

					enqueue_line(&state, format!("[acp] {}", line));
				}
			});
		}

		let handler = AcpMessageHandler {
			state: self.state.clone(),
		};

		let (conn, io_fut) =
			ClientSideConnection::new(handler, stdin.compat_write(), stdout.compat(), |fut| {
				tokio::task::spawn_local(fut);
			});

		let state = self.state.clone();
		tokio::task::spawn_local(async move {
			if let Err(e) = io_fut.await {
				enqueue_line(&state, format!("ACP IO error: {e:?}"));
			}
		});

		let conn = Arc::new(conn);

		let init_res = match conn
			.initialize(
				InitializeRequest::new(ProtocolVersion::from(1))
					.client_capabilities(
						ClientCapabilities::new()
							.fs(FileSystemCapability::new()
								.read_text_file(false)
								.write_text_file(false))
							.terminal(false),
					)
					.client_info(Implementation::new("Tome", "0.1.0").title("Tome Editor")),
			)
			.await
		{
			Ok(res) => res,
			Err(e) => {
				let status = child
					.try_wait()
					.ok()
					.flatten()
					.map(|s| s.to_string())
					.unwrap_or_else(|| "(still running)".to_string());
				let stderr_tail = format_stderr_tail(&stderr_tail);
				let msg = format!(
					"ACP initialize failed: err={e:?} child_status={status} stderr_tail={stderr_tail}"
				);
				self.enqueue_message(msg.clone());
				return Err(agent_client_protocol::Error::new(-32603, msg));
			}
		};

		let agent_info = init_res.agent_info.unwrap();
		self.enqueue_message(format!(
			"Connected to agent: {} (v{})",
			agent_info.name, init_res.protocol_version
		));

		let session_res = match conn.new_session(NewSessionRequest::new(cwd.clone())).await {
			Ok(res) => res,
			Err(e) => {
				let status = child
					.try_wait()
					.ok()
					.flatten()
					.map(|s| s.to_string())
					.unwrap_or_else(|| "(still running)".to_string());
				let stderr_tail = format_stderr_tail(&stderr_tail);
				let msg = format!(
					"ACP new_session failed: cwd={cwd:?} err={e:?} child_status={status} stderr_tail={stderr_tail}"
				);
				self.enqueue_message(msg.clone());
				return Err(agent_client_protocol::Error::new(-32603, msg));
			}
		};
		self.session_id = Some(session_res.session_id.to_string());

		// Extract and store model info from session response
		if let Some(models) = session_res.models {
			// Store available models
			{
				let mut available = self.state.available_models.lock();
				*available = models.available_models.clone();
			}
			// Store current model
			{
				let mut current = self.state.current_model.lock();
				*current = models.current_model_id.0.to_string();
			}
			self.enqueue_message(format!(
				"Session started (model: {})",
				models.current_model_id
			));
		} else {
			self.enqueue_message("Session started".to_string());
		}

		while let Some(cmd) = self.cmd_rx.recv().await {
			match cmd {
				AgentCommand::Prompt { content } => {
					// Clear last assistant text before new prompt
					{
						let mut last = self.state.last_assistant_text.lock();
						last.clear();
					}
					if let Some(session_id) = &self.session_id {
						let req = PromptRequest::new(
							session_id.clone(),
							vec![ContentBlock::Text(TextContent::new(content))],
						);
						let conn_clone = conn.clone();
						tokio::task::spawn_local(async move {
							let _ = conn_clone.prompt(req).await;
						});
					}
				}
				AgentCommand::Cancel => {
					if let Some(session_id) = &self.session_id {
						let _ = conn
							.cancel(CancelNotification::new(session_id.clone()))
							.await;
					}
				}
				AgentCommand::Stop => break,
				AgentCommand::Start { cwd: new_cwd } => {
					// Update workspace root if restarting with new cwd
					{
						let mut root = self.state.workspace_root.lock();
						*root = Some(new_cwd);
					}
					self.enqueue_message("Agent already started".to_string());
				}
				AgentCommand::SetModel { model_id } => {
					if let Some(session_id) = &self.session_id {
						let req = SetSessionModelRequest::new(session_id.clone(), model_id.clone());
						let conn_clone = conn.clone();
						let state = self.state.clone();
						let model_id_clone = model_id.clone();
						tokio::task::spawn_local(async move {
							match conn_clone.set_session_model(req).await {
								Ok(_) => {
									// Update local state with new model
									{
										let mut current = state.current_model.lock();
										*current = model_id_clone.clone();
									}
									enqueue_line(
										&state,
										format!("Model set to: {}", model_id_clone),
									);
								}
								Err(e) => {
									enqueue_line(&state, format!("Failed to set model: {:?}", e));
								}
							}
						});
					} else {
						self.enqueue_message("No active session".to_string());
					}
				}
			}
		}

		Ok(())
	}

	fn enqueue_message(&self, msg: String) {
		enqueue_line(&self.state, msg);
	}
}

/// Enqueue a message to the event queue.
pub fn enqueue_line(state: &AcpState, msg: String) {
	let msg = strip_ansi_and_controls(&msg);
	let mut events = state.events.lock();
	events.push(AcpEvent::ShowMessage(msg));
}

fn format_stderr_tail(stderr_tail: &Mutex<VecDeque<String>>) -> String {
	let tail = stderr_tail.lock();
	if tail.is_empty() {
		return "(empty)".to_string();
	}

	let mut joined = tail.iter().cloned().collect::<Vec<_>>().join(" | ");
	const MAX_LEN: usize = 800;
	if joined.len() > MAX_LEN {
		joined.truncate(MAX_LEN);
		joined.push_str("...");
	}
	format!("\"{}\"", joined)
}

/// Strip ANSI escape codes and control characters from a string.
pub fn strip_ansi_and_controls(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let mut chars = s.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '\u{1b}' {
			if matches!(chars.peek(), Some('[')) {
				let _ = chars.next();
				for c in chars.by_ref() {
					if ('@'..='~').contains(&c) {
						break;
					}
				}
			}
			continue;
		}

		if ch.is_control() {
			continue;
		}

		out.push(ch);
	}

	out
}
