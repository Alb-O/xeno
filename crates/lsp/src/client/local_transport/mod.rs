//! Local transport for spawning LSP servers as child processes.
//!
//! Manages language server processes directly using stdin/stdout JSON-RPC communication.

mod io;

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::Value as JsonValue;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

use super::config::{LanguageServerId, ServerConfig};
use super::transport::{LspTransport, StartedServer, TransportEvent, TransportStatus};
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId, ResponseError};
use crate::{Error, Result};

/// Outbound message envelope for total ordering and barrier support.
pub(super) enum Outbound {
	Notify {
		notif: AnyNotification,
		written: Option<oneshot::Sender<Result<()>>>,
	},
	Request {
		pending: PendingRequest,
	},
	Reply {
		reply: ReplyMsg,
		written: Option<oneshot::Sender<Result<()>>>,
	},
}

/// State for a running server process.
struct ServerProcess {
	/// The child process handle.
	child: Child,
	/// Channel for sending all outbound messages to the server.
	outbound_tx: mpsc::UnboundedSender<Outbound>,
}

/// A pending request awaiting a response.
pub(super) struct PendingRequest {
	pub(super) request: AnyRequest,
	pub(super) response_tx: oneshot::Sender<Result<AnyResponse>>,
}

/// A reply to a server-initiated request.
pub(super) struct ReplyMsg {
	pub(super) id: RequestId,
	pub(super) resp: std::result::Result<JsonValue, ResponseError>,
}

/// Local transport that spawns LSP servers as child processes.
///
/// Manages server processes using stdin/stdout JSON-RPC communication.
pub struct LocalTransport {
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

		let (outbound_tx, outbound_rx) = mpsc::unbounded_channel::<Outbound>();
		let event_tx = self.event_tx.clone();

		// Spawn the I/O task for this server
		tokio::spawn(io::run_server_io(id, stdin, stdout, outbound_rx, event_tx));

		Ok(ServerProcess { child, outbound_tx })
	}
}

impl Default for LocalTransport {
	fn default() -> Self {
		let (event_tx, event_rx) = mpsc::unbounded_channel();
		Self {
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
		let id = cfg.id;

		tracing::info!(
			server_id = %id,
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
			.outbound_tx
			.send(Outbound::Notify {
				notif,
				written: None,
			})
			.map_err(|_| Error::ServiceStopped)
	}

	async fn notify_with_barrier(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> Result<oneshot::Receiver<Result<()>>> {
		let (tx, rx) = oneshot::channel();
		let servers = self.servers.read();
		let process = servers
			.get(&server)
			.ok_or_else(|| Error::Protocol(format!("server {server:?} not found")))?;
		process
			.outbound_tx
			.send(Outbound::Notify {
				notif,
				written: Some(tx),
			})
			.map_err(|_| Error::ServiceStopped)?;
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
				.outbound_tx
				.send(Outbound::Request {
					pending: PendingRequest {
						request: req.clone(),
						response_tx,
					},
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
		server: LanguageServerId,
		id: RequestId,
		resp: std::result::Result<JsonValue, ResponseError>,
	) -> Result<()> {
		let servers = self.servers.read();
		let process = servers
			.get(&server)
			.ok_or_else(|| Error::Protocol(format!("server {server:?} not found")))?;
		process
			.outbound_tx
			.send(Outbound::Reply {
				reply: ReplyMsg { id, resp },
				written: None,
			})
			.map_err(|_| Error::ServiceStopped)
	}

	async fn stop(&self, server: LanguageServerId) -> Result<()> {
		let proc = {
			let mut servers = self.servers.write();
			servers.remove(&server)
		};

		let Some(mut proc) = proc else {
			return Ok(()); // idempotent
		};

		// Best-effort kill, then wait a bit.
		let _ = proc.child.start_kill();
		let _ = tokio::time::timeout(Duration::from_secs(2), proc.child.wait()).await;

		Ok(())
	}
}
