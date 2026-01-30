//! Local LSP transport implementation.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{error, warn};

use super::config::{LanguageServerId, ServerConfig};
use super::event_handler::{LogLevel, LspEventHandler};
use super::outbox::{OUTBOUND_QUEUE_LEN, OutboundMsg, outbound_dispatcher};
use super::router_setup::{ClientState, build_router};
use super::state::ServerState;
use crate::client::transport::{LspTransport, StartedServer, TransportEvent, TransportStatus};
use crate::{
	AnyNotification, AnyRequest, AnyResponse, JsonValue, MainLoop, Message, ResponseError, Result,
};

/// Local LSP transport that spawns servers as child processes.
pub struct LocalTransport {
	/// Generator for server IDs.
	next_id: AtomicU64,
	/// Channel for outbound transport events.
	events_tx: mpsc::UnboundedSender<TransportEvent>,
	/// Shared receiver for transport events (single-take).
	events_rx: Mutex<Option<mpsc::UnboundedReceiver<TransportEvent>>>,
	/// Map of active local server instances.
	servers: DashMap<LanguageServerId, LocalInstance>,
}

#[derive(Clone)]
struct LocalInstance {
	outbound_tx: mpsc::Sender<OutboundMsg>,
	#[allow(dead_code)]
	state_rx: watch::Receiver<ServerState>,
}

impl LocalTransport {
	/// Create a new local transport instance.
	pub fn new() -> Arc<Self> {
		let (tx, rx) = mpsc::unbounded_channel();
		Arc::new(Self {
			next_id: AtomicU64::new(1),
			events_tx: tx,
			events_rx: Mutex::new(Some(rx)),
			servers: DashMap::new(),
		})
	}
}

#[async_trait]
impl LspTransport for LocalTransport {
	fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
		self.events_rx
			.lock()
			.unwrap()
			.take()
			.expect("events() called twice on LocalTransport")
	}

	async fn start(&self, cfg: ServerConfig) -> Result<StartedServer> {
		let id = LanguageServerId(self.next_id.fetch_add(1, Ordering::Relaxed));

		let mut cmd = Command::new(&cfg.command);
		cmd.args(&cfg.args)
			.envs(&cfg.env)
			.stdin(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.current_dir(&cfg.root_path)
			.kill_on_drop(true);

		#[cfg(unix)]
		cmd.process_group(0);

		let mut process = cmd.spawn().map_err(|e| crate::Error::ServerSpawn {
			server: cfg.command.clone(),
			reason: e.to_string(),
		})?;

		let stdin = process.stdin.take().expect("stdin");
		let stdout = process.stdout.take().expect("stdout");
		let stderr = process.stderr.take().expect("stderr");

		// Stderr monitoring
		tokio::spawn({
			let server_id = id;
			async move {
				use tokio::io::AsyncBufReadExt;
				let reader = tokio::io::BufReader::new(stderr);
				let mut lines = reader.lines();
				while let Ok(Some(line)) = lines.next_line().await {
					warn!(server_id = server_id.0, stderr = %line, "LSP server stderr");
				}
			}
		});

		let (state_tx, state_rx) = watch::channel(ServerState::Starting);
		let handler = Arc::new(TransportEventHandler {
			server: id,
			tx: self.events_tx.clone(),
		});

		let state = Arc::new(ClientState::new(id, handler));
		let (main_loop, socket) = MainLoop::new_client(|_socket| build_router(state));

		let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_LEN);
		tokio::spawn(outbound_dispatcher(outbound_rx, socket, state_rx.clone()));

		// Monitor lifecycle
		let events_tx = self.events_tx.clone();
		let mut monitor_state_rx = state_rx.clone();
		tokio::spawn(async move {
			while monitor_state_rx.changed().await.is_ok() {
				let status = match *monitor_state_rx.borrow() {
					ServerState::Starting => TransportStatus::Starting,
					ServerState::Ready => TransportStatus::Running,
					ServerState::Dead => break,
				};
				let _ = events_tx.send(TransportEvent::Status { server: id, status });
			}
		});

		// Main loop task
		let events_tx = self.events_tx.clone();
		let servers = self.servers.clone();
		tokio::spawn(async move {
			let result = main_loop.run_buffered(stdout, stdin).await;
			if let Err(ref e) = result {
				error!(server_id = id.0, error = %e, "LSP main loop error");
			}

			let _ = state_tx.send(ServerState::Dead);
			let status = if result.is_ok() {
				TransportStatus::Stopped
			} else {
				TransportStatus::Crashed
			};
			let _ = events_tx.send(TransportEvent::Status { server: id, status });

			servers.remove(&id);
			drop(process);
		});

		self.servers.insert(
			id,
			LocalInstance {
				outbound_tx: outbound_tx.clone(),
				state_rx,
			},
		);

		Ok(StartedServer { id })
	}

	async fn notify(&self, server: LanguageServerId, notif: AnyNotification) -> Result<()> {
		let instance = self
			.servers
			.get(&server)
			.ok_or_else(|| crate::Error::ServiceStopped)?;
		instance
			.outbound_tx
			.send(OutboundMsg::Notification {
				notification: notif,
				barrier: None,
			})
			.await
			.map_err(|_| crate::Error::ServiceStopped)
	}

	async fn notify_with_barrier(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> Result<oneshot::Receiver<()>> {
		let instance = self
			.servers
			.get(&server)
			.ok_or_else(|| crate::Error::ServiceStopped)?;
		let (tx, rx) = oneshot::channel();
		instance
			.outbound_tx
			.send(OutboundMsg::Notification {
				notification: notif,
				barrier: Some(tx),
			})
			.await
			.map_err(|_| crate::Error::ServiceStopped)?;
		Ok(rx)
	}

	async fn request(
		&self,
		server: LanguageServerId,
		req: AnyRequest,
		timeout_dur: Option<Duration>,
	) -> Result<AnyResponse> {
		let instance = self
			.servers
			.get(&server)
			.ok_or_else(|| crate::Error::ServiceStopped)?;
		let (tx, rx) = oneshot::channel();
		instance
			.outbound_tx
			.send(OutboundMsg::Request {
				request: req,
				response_tx: tx,
			})
			.await
			.map_err(|_| crate::Error::ServiceStopped)?;

		let timeout_dur = timeout_dur.unwrap_or(Duration::from_secs(30));
		match tokio::time::timeout(timeout_dur, rx).await {
			Ok(Ok(resp)) => Ok(resp),
			Ok(Err(_)) => Err(crate::Error::ServiceStopped),
			Err(_) => Err(crate::Error::RequestTimeout("local".into())),
		}
	}

	async fn reply(
		&self,
		_server: LanguageServerId,
		_resp: Result<JsonValue, ResponseError>,
	) -> Result<()> {
		Ok(())
	}
}

struct TransportEventHandler {
	server: LanguageServerId,
	tx: mpsc::UnboundedSender<TransportEvent>,
}

impl LspEventHandler for TransportEventHandler {
	fn on_diagnostics(
		&self,
		server_id: LanguageServerId,
		uri: lsp_types::Uri,
		diags: Vec<lsp_types::Diagnostic>,
		version: Option<i32>,
	) {
		let diagnostics = serde_json::to_value(&diags).unwrap_or(JsonValue::Array(vec![]));
		let version_u32 = version.unwrap_or(0).max(0) as u32;

		let _ = self.tx.send(TransportEvent::Diagnostics {
			server: server_id,
			uri: uri.to_string(),
			version: version_u32,
			diagnostics: diagnostics.clone(),
		});

		// Also emit as raw LSP notification for parity
		let params = serde_json::json!({
			"uri": uri.to_string(),
			"diagnostics": diagnostics,
			"version": version,
		});
		let _ = self.tx.send(TransportEvent::Message {
			server: server_id,
			message: Message::Notification(AnyNotification {
				method: "textDocument/publishDiagnostics".into(),
				params,
			}),
		});
	}

	fn on_progress(&self, server_id: LanguageServerId, params: lsp_types::ProgressParams) {
		let _ = self.tx.send(TransportEvent::Message {
			server: server_id,
			message: Message::Notification(AnyNotification {
				method: "$/progress".into(),
				params: serde_json::to_value(params).unwrap_or(JsonValue::Null),
			}),
		});
	}

	fn on_log_message(&self, server_id: LanguageServerId, level: LogLevel, message: &str) {
		let typ = match level {
			LogLevel::Error => lsp_types::MessageType::ERROR,
			LogLevel::Warning => lsp_types::MessageType::WARNING,
			LogLevel::Info => lsp_types::MessageType::INFO,
			LogLevel::Debug => lsp_types::MessageType::LOG,
		};
		let params = lsp_types::LogMessageParams {
			typ,
			message: message.to_string(),
		};
		let _ = self.tx.send(TransportEvent::Message {
			server: server_id,
			message: Message::Notification(AnyNotification {
				method: "window/logMessage".into(),
				params: serde_json::to_value(params).unwrap_or(JsonValue::Null),
			}),
		});
	}

	fn on_show_message(&self, server_id: LanguageServerId, level: LogLevel, message: &str) {
		let typ = match level {
			LogLevel::Error => lsp_types::MessageType::ERROR,
			LogLevel::Warning => lsp_types::MessageType::WARNING,
			LogLevel::Info => lsp_types::MessageType::INFO,
			LogLevel::Debug => lsp_types::MessageType::LOG,
		};
		let params = lsp_types::ShowMessageParams {
			typ,
			message: message.to_string(),
		};
		let _ = self.tx.send(TransportEvent::Message {
			server: server_id,
			message: Message::Notification(AnyNotification {
				method: "window/showMessage".into(),
				params: serde_json::to_value(params).unwrap_or(JsonValue::Null),
			}),
		});
	}
}
