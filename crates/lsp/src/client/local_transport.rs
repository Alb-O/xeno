//! Local transport for spawning LSP servers as child processes.
//!
//! Manages language server processes directly using stdin/stdout JSON-RPC communication.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

use super::config::{LanguageServerId, ServerConfig};
use super::transport::{LspTransport, StartedServer, TransportEvent, TransportStatus};
use crate::protocol::JsonRpcProtocol;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId, ResponseError};
use crate::{Error, Result};

/// State for a running server process.
struct ServerProcess {
	/// The child process handle.
	#[allow(dead_code)]
	child: Child,
	/// Channel for sending requests to the server.
	request_tx: mpsc::UnboundedSender<PendingRequest>,
	/// Channel for sending notifications to the server.
	notify_tx: mpsc::UnboundedSender<AnyNotification>,
}

/// A pending request awaiting a response.
struct PendingRequest {
	request: AnyRequest,
	response_tx: oneshot::Sender<Result<AnyResponse>>,
}

/// Local transport that spawns LSP servers as child processes.
///
/// Manages server processes using stdin/stdout JSON-RPC communication.
pub struct LocalTransport {
	/// Counter for generating unique server IDs.
	next_id: AtomicU64,
	/// Active server processes.
	servers: RwLock<HashMap<LanguageServerId, ServerProcess>>,
	/// Channel for emitting transport events to the manager.
	event_tx: mpsc::UnboundedSender<TransportEvent>,
	/// Receiver template for cloning (only used once in spawn_router).
	event_rx: RwLock<Option<mpsc::UnboundedReceiver<TransportEvent>>>,
}

impl LocalTransport {
	/// Create a new local transport.
	pub fn new() -> Arc<Self> {
		let (event_tx, event_rx) = mpsc::unbounded_channel();
		Arc::new(Self {
			next_id: AtomicU64::new(1),
			servers: RwLock::new(HashMap::new()),
			event_tx,
			event_rx: RwLock::new(Some(event_rx)),
		})
	}

	/// Spawn a server process and set up communication channels.
	async fn spawn_server(
		&self,
		id: LanguageServerId,
		cfg: &ServerConfig,
	) -> Result<ServerProcess> {
		let mut cmd = Command::new(&cfg.command);
		cmd.args(&cfg.args)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.kill_on_drop(true);

		for (key, value) in &cfg.env {
			cmd.env(key, value);
		}

		cmd.current_dir(&cfg.root_path);

		let mut child = cmd.spawn().map_err(|e| Error::ServerSpawn {
			server: cfg.command.clone(),
			reason: e.to_string(),
		})?;

		let stdin = child.stdin.take().ok_or_else(|| Error::ServerSpawn {
			server: cfg.command.clone(),
			reason: "failed to capture stdin".into(),
		})?;
		let stdout = child.stdout.take().ok_or_else(|| Error::ServerSpawn {
			server: cfg.command.clone(),
			reason: "failed to capture stdout".into(),
		})?;

		let (request_tx, request_rx) = mpsc::unbounded_channel::<PendingRequest>();
		let (notify_tx, notify_rx) = mpsc::unbounded_channel::<AnyNotification>();
		let event_tx = self.event_tx.clone();

		// Spawn the I/O task for this server
		tokio::spawn(run_server_io(
			id, stdin, stdout, request_rx, notify_rx, event_tx,
		));

		Ok(ServerProcess {
			child,
			request_tx,
			notify_tx,
		})
	}
}

impl Default for LocalTransport {
	fn default() -> Self {
		let (event_tx, event_rx) = mpsc::unbounded_channel();
		Self {
			next_id: AtomicU64::new(1),
			servers: RwLock::new(HashMap::new()),
			event_tx,
			event_rx: RwLock::new(Some(event_rx)),
		}
	}
}

#[async_trait]
impl LspTransport for LocalTransport {
	fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
		self.event_rx
			.write()
			.take()
			.expect("events() can only be called once")
	}

	async fn start(&self, cfg: ServerConfig) -> Result<StartedServer> {
		let id = LanguageServerId(self.next_id.fetch_add(1, Ordering::Relaxed));

		tracing::info!(
			server_id = id.0,
			command = %cfg.command,
			"Starting local LSP server"
		);

		let _ = self.event_tx.send(TransportEvent::Status {
			server: id,
			status: TransportStatus::Starting,
		});

		let process = self.spawn_server(id, &cfg).await?;
		self.servers.write().insert(id, process);

		let _ = self.event_tx.send(TransportEvent::Status {
			server: id,
			status: TransportStatus::Running,
		});

		Ok(StartedServer { id })
	}

	async fn notify(&self, server: LanguageServerId, notif: AnyNotification) -> Result<()> {
		let servers = self.servers.read();
		let process = servers
			.get(&server)
			.ok_or_else(|| Error::Protocol(format!("server {server:?} not found")))?;
		process
			.notify_tx
			.send(notif)
			.map_err(|_| Error::ServiceStopped)
	}

	async fn notify_with_barrier(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> Result<oneshot::Receiver<()>> {
		// For local transport, notifications are sent immediately
		self.notify(server, notif).await?;
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(());
		Ok(rx)
	}

	async fn request(
		&self,
		server: LanguageServerId,
		req: AnyRequest,
		timeout: Option<Duration>,
	) -> Result<AnyResponse> {
		let (response_tx, response_rx) = oneshot::channel();

		{
			let servers = self.servers.read();
			let process = servers
				.get(&server)
				.ok_or_else(|| Error::Protocol(format!("server {server:?} not found")))?;
			process
				.request_tx
				.send(PendingRequest {
					request: req.clone(),
					response_tx,
				})
				.map_err(|_| Error::ServiceStopped)?;
		}

		let timeout_duration = timeout.unwrap_or(Duration::from_secs(30));
		match tokio::time::timeout(timeout_duration, response_rx).await {
			Ok(Ok(result)) => result,
			Ok(Err(_)) => Err(Error::ServiceStopped),
			Err(_) => Err(Error::RequestTimeout(req.method)),
		}
	}

	async fn reply(
		&self,
		_server: LanguageServerId,
		_resp: std::result::Result<JsonValue, ResponseError>,
	) -> Result<()> {
		// Server-initiated requests are not yet supported in local transport
		Ok(())
	}
}

/// Runs the I/O loop for a single server process.
async fn run_server_io(
	id: LanguageServerId,
	mut stdin: tokio::process::ChildStdin,
	stdout: tokio::process::ChildStdout,
	mut request_rx: mpsc::UnboundedReceiver<PendingRequest>,
	mut notify_rx: mpsc::UnboundedReceiver<AnyNotification>,
	event_tx: mpsc::UnboundedSender<TransportEvent>,
) {
	let mut reader = BufReader::new(stdout);
	let mut pending: HashMap<RequestId, oneshot::Sender<Result<AnyResponse>>> = HashMap::new();
	let protocol = JsonRpcProtocol::new();
	let mut read_buf = String::new();

	loop {
		tokio::select! {
			// Handle outbound requests
			Some(pending_req) = request_rx.recv() => {
				let req_id = pending_req.request.id.clone();
				if let Err(e) = write_message(&mut stdin, &protocol, &pending_req.request).await {
					let _ = pending_req.response_tx.send(Err(e));
					continue;
				}
				pending.insert(req_id, pending_req.response_tx);
			}

			// Handle outbound notifications
			Some(notif) = notify_rx.recv() => {
				if let Err(e) = write_notification(&mut stdin, &protocol, &notif).await {
					tracing::warn!(server_id = id.0, error = %e, "Failed to send notification");
				}
			}

			// Handle inbound messages from server
			result = read_message(&mut reader, &protocol, &mut read_buf) => {
				match result {
					Ok(Some(msg)) => {
						handle_inbound_message(id, msg, &mut pending, &event_tx);
					}
					Ok(None) => {
						// EOF - server stopped
						tracing::info!(server_id = id.0, "LSP server closed connection");
						let _ = event_tx.send(TransportEvent::Status {
							server: id,
							status: TransportStatus::Stopped,
						});
						break;
					}
					Err(e) => {
						tracing::error!(server_id = id.0, error = %e, "Error reading from LSP server");
						let _ = event_tx.send(TransportEvent::Status {
							server: id,
							status: TransportStatus::Crashed,
						});
						break;
					}
				}
			}
		}
	}

	// Clean up pending requests
	for (_, tx) in pending {
		let _ = tx.send(Err(Error::ServiceStopped));
	}

	let _ = event_tx.send(TransportEvent::Disconnected);
}

/// Writes a JSON-RPC request to the server's stdin.
async fn write_message(
	stdin: &mut tokio::process::ChildStdin,
	_protocol: &JsonRpcProtocol,
	req: &AnyRequest,
) -> Result<()> {
	let json = serde_json::to_string(&serde_json::json!({
		"jsonrpc": "2.0",
		"id": req.id,
		"method": req.method,
		"params": req.params,
	}))?;

	let msg = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
	stdin.write_all(msg.as_bytes()).await?;
	stdin.flush().await?;
	Ok(())
}

/// Writes a JSON-RPC notification to the server's stdin.
async fn write_notification(
	stdin: &mut tokio::process::ChildStdin,
	_protocol: &JsonRpcProtocol,
	notif: &AnyNotification,
) -> Result<()> {
	let json = serde_json::to_string(&serde_json::json!({
		"jsonrpc": "2.0",
		"method": notif.method,
		"params": notif.params,
	}))?;

	let msg = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
	stdin.write_all(msg.as_bytes()).await?;
	stdin.flush().await?;
	Ok(())
}

/// Reads a JSON-RPC message from the server's stdout.
async fn read_message(
	reader: &mut BufReader<tokio::process::ChildStdout>,
	_protocol: &JsonRpcProtocol,
	buf: &mut String,
) -> Result<Option<JsonValue>> {
	// Read headers
	let mut content_length: Option<usize> = None;
	loop {
		buf.clear();
		let bytes_read = reader.read_line(buf).await?;
		if bytes_read == 0 {
			return Ok(None); // EOF
		}

		let line = buf.trim();
		if line.is_empty() {
			break;
		}

		if let Some(len_str) = line.strip_prefix("Content-Length: ") {
			content_length = len_str.parse().ok();
		}
	}

	let length = content_length.ok_or_else(|| Error::Protocol("missing Content-Length".into()))?;

	// Read body
	let mut body = vec![0u8; length];
	tokio::io::AsyncReadExt::read_exact(reader, &mut body).await?;

	let json: JsonValue = serde_json::from_slice(&body)?;
	Ok(Some(json))
}

/// Handles an inbound message from the server.
fn handle_inbound_message(
	id: LanguageServerId,
	msg: JsonValue,
	pending: &mut HashMap<RequestId, oneshot::Sender<Result<AnyResponse>>>,
	event_tx: &mpsc::UnboundedSender<TransportEvent>,
) {
	// Check if it's a response (has "id" but no "method")
	if msg.get("id").is_some() && msg.get("method").is_none() {
		let resp: AnyResponse = match serde_json::from_value(msg) {
			Ok(r) => r,
			Err(e) => {
				tracing::warn!(server_id = id.0, error = %e, "Failed to parse response");
				return;
			}
		};

		if let Some(tx) = pending.remove(&resp.id) {
			let _ = tx.send(Ok(resp));
		}
		return;
	}

	// Check if it's a notification (has "method" but no "id")
	if msg.get("method").is_some() && msg.get("id").is_none() {
		let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
		let params = msg.get("params").cloned().unwrap_or(JsonValue::Null);

		// Handle diagnostics specially
		if method == "textDocument/publishDiagnostics"
			&& let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
				let version = params
					.get("version")
					.and_then(|v| v.as_u64())
					.map(|v| v as u32);
				let diagnostics = params
					.get("diagnostics")
					.cloned()
					.unwrap_or(JsonValue::Array(vec![]));
				let _ = event_tx.send(TransportEvent::Diagnostics {
					server: id,
					uri: uri.to_string(),
					version,
					diagnostics,
				});
				return;
			}

		// Other notifications go through as messages
		let _ = event_tx.send(TransportEvent::Message {
			server: id,
			message: crate::Message::Notification(AnyNotification {
				method: method.to_string(),
				params,
			}),
		});
		return;
	}

	// It's a server-initiated request (has both "id" and "method")
	if msg.get("id").is_some() && msg.get("method").is_some() {
		let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
		let params = msg.get("params").cloned().unwrap_or(JsonValue::Null);
		let req_id = msg.get("id").cloned().unwrap_or(JsonValue::Null);

		let id_parsed = match req_id {
			JsonValue::Number(n) => RequestId::Number(n.as_i64().unwrap_or(0) as i32),
			JsonValue::String(s) => RequestId::String(s),
			_ => RequestId::Number(0),
		};

		let _ = event_tx.send(TransportEvent::Message {
			server: id,
			message: crate::Message::Request(AnyRequest {
				id: id_parsed,
				method: method.to_string(),
				params,
			}),
		});
	}
}
