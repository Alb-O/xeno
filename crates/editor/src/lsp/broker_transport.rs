//! Broker-based LSP transport implementation.
//!
//! This transport is Unix-only due to its use of Unix domain sockets.

#![cfg(unix)]

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use xeno_broker_proto::BrokerProtocol;
use xeno_broker_proto::types::{
	ErrorCode, Event, IpcFrame, Request, RequestPayload, Response, ResponsePayload, ServerId,
	SessionId,
};
use xeno_lsp::client::LanguageServerId;
use xeno_lsp::client::transport::{LspTransport, StartedServer, TransportEvent, TransportStatus};
use xeno_lsp::{
	AnyNotification, AnyRequest, AnyResponse, JsonValue, Message, ResponseError, Result,
};
use xeno_rpc::{AnyEvent, MainLoop, MainLoopEvent, PeerSocket, RpcService};

/// Transport that communicates with a Xeno broker over a Unix socket.
///
/// This transport manages the lifecycle of the editor's connection to the broker
/// daemon. If the broker is not running, it attempts to spawn it automatically
/// as a detached background process.
///
/// # Environment Variables
///
/// - `XENO_BROKER_BIN`: Path to the `xeno-broker` binary (overrides resolution).
/// - `XENO_BROKER_SOCKET`: Custom path for the IPC socket.
pub struct BrokerTransport {
	session_id: SessionId,
	socket_path: std::path::PathBuf,
	events_tx: mpsc::UnboundedSender<TransportEvent>,
	events_rx: std::sync::Mutex<Option<mpsc::UnboundedReceiver<TransportEvent>>>,
	rpc: Arc<tokio::sync::Mutex<Option<BrokerRpc>>>,

	/// FIFO queues for pipelined server-initiated requests (one queue per LSP server).
	pending_server_requests: Arc<DashMap<ServerId, VecDeque<xeno_lsp::RequestId>>>,

	/// Internal routing for shared state events received via the broker IPC.
	shared_state_tx: mpsc::UnboundedSender<crate::shared_state::SharedStateEvent>,
	/// One-time receiver for shared state events, taken by the editor main loop.
	shared_state_rx:
		std::sync::Mutex<Option<mpsc::UnboundedReceiver<crate::shared_state::SharedStateEvent>>>,
}

struct BrokerRpc {
	socket: PeerSocket<IpcFrame, Request, Response>,
}

impl BrokerTransport {
	/// Create a new broker transport targeting the default socket path.
	#[must_use]
	pub fn new() -> Arc<Self> {
		Self::with_socket_path(xeno_broker_proto::paths::default_socket_path())
	}

	/// Create a new broker transport with a specific socket path and session ID.
	#[must_use]
	pub fn with_socket_and_session(
		socket_path: std::path::PathBuf,
		session_id: SessionId,
	) -> Arc<Self> {
		let (tx, rx) = mpsc::unbounded_channel();
		let (bs_tx, bs_rx) = mpsc::unbounded_channel();
		Arc::new(Self {
			session_id,
			socket_path,
			events_tx: tx,
			events_rx: std::sync::Mutex::new(Some(rx)),
			rpc: Arc::new(tokio::sync::Mutex::new(None)),
			pending_server_requests: Arc::new(DashMap::new()),
			shared_state_tx: bs_tx,
			shared_state_rx: std::sync::Mutex::new(Some(bs_rx)),
		})
	}

	/// Returns the unique session ID associated with this transport connection.
	pub fn session_id(&self) -> SessionId {
		self.session_id
	}

	/// Takes the shared state event receiver.
	///
	/// # Panics
	///
	/// Panics if called more than once. The receiver must be managed by the
	/// editor main loop for the lifetime of the session.
	pub fn take_shared_state_events(
		&self,
	) -> Option<mpsc::UnboundedReceiver<crate::shared_state::SharedStateEvent>> {
		self.shared_state_rx.lock().unwrap().take()
	}

	/// Sends a shared state request to the broker and awaits the response.
	///
	/// # Errors
	///
	/// Returns a protocol error if the broker is unreachable or if the request
	/// fails at the service layer.
	pub async fn shared_state_request(
		&self,
		payload: RequestPayload,
	) -> xeno_lsp::Result<ResponsePayload> {
		self.handle_rpc_result(self.shared_state_request_raw(payload).await, "SharedState")
			.await
	}

	/// Sends a shared state request and returns broker error codes directly.
	pub async fn shared_state_request_raw(
		&self,
		payload: RequestPayload,
	) -> std::result::Result<ResponsePayload, ErrorCode> {
		let rpc = self
			.ensure_connected(&self.socket_path)
			.await
			.map_err(|err| {
				tracing::warn!(error = %err, "shared state connect failed");
				ErrorCode::Internal
			})?;
		rpc.call(payload, Duration::from_secs(5)).await
	}

	fn with_socket_path(socket_path: std::path::PathBuf) -> Arc<Self> {
		// Random session ID prevents collisions if PIDs are reused or if multiple
		// editors run in a shared environment (e.g. Nix builds, containers).
		let session_id = SessionId(uuid::Uuid::new_v4().as_u64_pair().0);
		Self::with_socket_and_session(socket_path, session_id)
	}

	/// Establishes a connection to the broker, spawning it if necessary.
	async fn ensure_connected(&self, socket_path: &std::path::Path) -> Result<BrokerRpc> {
		let mut rpc_lock = self.rpc.lock().await;
		if let Some(rpc) = &*rpc_lock {
			return Ok(rpc.clone());
		}

		let stream = self.connect_or_spawn(socket_path).await?;
		let (r, w) = stream.into_split();

		let (main_loop, socket) = MainLoop::new(
			{
				let events_tx = self.events_tx.clone();
				let pending = self.pending_server_requests.clone();
				let shared_state_tx = self.shared_state_tx.clone();
				move |_| BrokerClientService {
					tx: events_tx.clone(),
					pending: pending.clone(),
					shared_state_tx: shared_state_tx.clone(),
				}
			},
			BrokerProtocol::new(),
			xeno_rpc::CounterIdGen::new(),
		);

		let rpc_weak = Arc::downgrade(&self.rpc);
		let pending_weak = Arc::downgrade(&self.pending_server_requests);
		let events_tx = self.events_tx.clone();

		tokio::spawn(async move {
			if let Err(e) = main_loop.run(tokio::io::BufReader::new(r), w).await {
				tracing::error!(error = %e, "Broker client mainloop failed");
			}

			if let Some(rpc_arc) = rpc_weak.upgrade() {
				*rpc_arc.lock().await = None;
			}
			if let Some(pending_arc) = pending_weak.upgrade() {
				pending_arc.clear();
			}

			let _ = events_tx.send(TransportEvent::Disconnected);
		});

		let rpc = BrokerRpc { socket };

		rpc.call(
			RequestPayload::Subscribe {
				session_id: self.session_id,
			},
			Duration::from_secs(5),
		)
		.await
		.map_err(|e| xeno_lsp::Error::Protocol(format!("Subscribe failed: {e:?}")))?;

		*rpc_lock = Some(rpc.clone());
		Ok(rpc)
	}

	/// Converts a broker RPC result into a transport [`Result`].
	async fn handle_rpc_result(
		&self,
		result: std::result::Result<ResponsePayload, ErrorCode>,
		op: &str,
	) -> Result<ResponsePayload> {
		match result {
			Ok(payload) => Ok(payload),
			Err(e) => Err(xeno_lsp::Error::Protocol(format!("{op} failed: {e:?}"))),
		}
	}

	/// Connect to the broker, spawning the daemon if necessary.
	///
	/// Automatically handles stale socket files from crashed broker processes.
	async fn connect_or_spawn(&self, socket_path: &std::path::Path) -> Result<UnixStream> {
		match UnixStream::connect(socket_path).await {
			Ok(s) => Ok(s),
			Err(e)
				if matches!(
					e.kind(),
					std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
				) =>
			{
				self.ensure_broker_running(socket_path).await?;
				UnixStream::connect(socket_path).await.map_err(|e2| {
					xeno_lsp::Error::Protocol(format!(
						"broker connect failed after spawn: path={} err={}",
						socket_path.display(),
						e2
					))
				})
			}
			Err(e) => Err(xeno_lsp::Error::Protocol(format!(
				"broker connect failed: path={} err={}",
				socket_path.display(),
				e
			))),
		}
	}

	/// Ensure the broker daemon is running, coordinating startup across multiple processes.
	///
	/// Uses an exclusive file lock to prevent race conditions. Double-checks connectivity
	/// under the lock, removes stale socket files, and spawns the daemon if needed.
	///
	/// # Errors
	///
	/// Returns a protocol error if the broker fails to spawn or become ready within 3 seconds.
	async fn ensure_broker_running(&self, socket_path: &std::path::Path) -> Result<()> {
		let lock_path = socket_path.with_extension("lock");
		let lock_file = std::fs::OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(false)
			.open(&lock_path)
			.map_err(xeno_lsp::Error::Io)?;

		use fs2::FileExt;
		lock_file.lock_exclusive().map_err(xeno_lsp::Error::Io)?;

		// Double-check under lock
		if UnixStream::connect(socket_path).await.is_ok() {
			let _ = lock_file.unlock();
			return Ok(());
		}

		if socket_path.exists() {
			let _ = std::fs::remove_file(socket_path);
		}

		self.spawn_broker_daemon(socket_path).await?;

		let deadline = Instant::now() + Duration::from_secs(3);
		while Instant::now() < deadline {
			if UnixStream::connect(socket_path).await.is_ok() {
				let _ = lock_file.unlock();
				return Ok(());
			}
			tokio::time::sleep(Duration::from_millis(20)).await;
		}

		let _ = lock_file.unlock();
		Err(xeno_lsp::Error::Protocol("broker spawn timeout".into()))
	}

	/// Spawn the broker daemon as a detached background process.
	///
	/// Stdio is suppressed to avoid cluttering the terminal.
	/// Spawn the broker daemon as a detached background process.
	///
	/// Propagates logging and debugging environment variables to enable file-based tracing
	/// in smoke tests and production debugging scenarios.
	async fn spawn_broker_daemon(&self, socket_path: &std::path::Path) -> Result<()> {
		let bin = resolve_broker_bin();

		let mut cmd = tokio::process::Command::new(&bin);
		cmd.arg("--socket")
			.arg(socket_path)
			.stdin(std::process::Stdio::null())
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null());

		for k in ["XENO_LOG_DIR", "RUST_LOG", "XENO_LOG", "RUST_BACKTRACE"] {
			if let Ok(v) = std::env::var(k) {
				cmd.env(k, v);
			}
		}

		let mut child = cmd.spawn().map_err(|e| {
			xeno_lsp::Error::Protocol(format!(
				"failed to spawn broker '{}': {} (checked XENO_BROKER_BIN, sibling, then PATH)",
				bin.display(),
				e
			))
		})?;

		tokio::spawn(async move {
			let _ = child.wait().await;
		});

		Ok(())
	}
}

/// Resolve the path to the `xeno-broker` binary.
///
/// Search order: `XENO_BROKER_BIN` env var, sibling to current executable, system `PATH`.
fn resolve_broker_bin() -> std::path::PathBuf {
	if let Ok(val) = std::env::var("XENO_BROKER_BIN") {
		return std::path::PathBuf::from(val);
	}

	let bin_name = if cfg!(windows) {
		"xeno-broker.exe"
	} else {
		"xeno-broker"
	};

	if let Ok(exe) = std::env::current_exe()
		&& let Some(dir) = exe.parent()
	{
		let candidate = dir.join(bin_name);
		if candidate.exists() {
			return candidate;
		}
	}

	std::path::PathBuf::from(bin_name)
}

impl BrokerRpc {
	async fn call(
		&self,
		payload: RequestPayload,
		timeout_dur: Duration,
	) -> std::result::Result<ResponsePayload, ErrorCode> {
		let req = Request {
			id: xeno_broker_proto::types::RequestId(0),
			payload,
		};
		let (tx, rx) = oneshot::channel::<Response>();

		// Check if send succeeded; fail fast on disconnect
		if self
			.socket
			.send(MainLoopEvent::OutgoingRequest(req, tx))
			.is_err()
		{
			return Err(ErrorCode::Internal);
		}

		let resp = match timeout(timeout_dur, rx).await {
			Ok(Ok(resp)) => resp,
			_ => return Err(ErrorCode::Timeout),
		};

		if let Some(err) = resp.error {
			return Err(err);
		}
		resp.payload.ok_or(ErrorCode::Internal)
	}
}

#[async_trait]
impl LspTransport for BrokerTransport {
	fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
		self.events_rx
			.lock()
			.unwrap()
			.take()
			.expect("events() called twice")
	}

	async fn start(&self, cfg: xeno_lsp::client::ServerConfig) -> Result<StartedServer> {
		let rpc = self.ensure_connected(&self.socket_path).await?;

		let broker_cfg = xeno_broker_proto::types::LspServerConfig {
			command: cfg.command.clone(),
			args: cfg.args.clone(),
			env: cfg
				.env
				.iter()
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect(),
			cwd: Some(cfg.root_path.to_string_lossy().to_string()),
		};

		tracing::trace!(
			command = %broker_cfg.command,
			cwd = ?broker_cfg.cwd,
			"BrokerTransport: requesting LspStart"
		);

		let resp = self
			.handle_rpc_result(
				rpc.call(
					RequestPayload::LspStart { config: broker_cfg },
					Duration::from_secs(30),
				)
				.await,
				"LspStart",
			)
			.await?;

		if let ResponsePayload::LspStarted { server_id } = resp {
			tracing::trace!(
				server_id = server_id.0,
				"BrokerTransport: LspStart returned server_id"
			);
			Ok(StartedServer {
				id: LanguageServerId(server_id.0),
			})
		} else {
			Err(xeno_lsp::Error::Protocol(
				"unexpected response to LspStart".into(),
			))
		}
	}

	async fn notify(&self, server: LanguageServerId, notif: AnyNotification) -> Result<()> {
		let rpc = self.ensure_connected(&self.socket_path).await?;
		let msg = Message::Notification(notif);
		let json =
			serde_json::to_string(&msg).map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;

		self.handle_rpc_result(
			rpc.call(
				RequestPayload::LspSend {
					session_id: self.session_id,
					server_id: ServerId(server.0),
					message: json,
				},
				Duration::from_secs(5),
			)
			.await,
			"LspSend",
		)
		.await?;
		Ok(())
	}

	async fn notify_with_barrier(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> Result<oneshot::Receiver<()>> {
		self.notify(server, notif).await?;
		let (tx, rx) = oneshot::channel::<()>();
		let _ = tx.send(());
		Ok(rx)
	}

	async fn request(
		&self,
		server: LanguageServerId,
		req: AnyRequest,
		timeout: Option<Duration>,
	) -> Result<AnyResponse> {
		let rpc = self.ensure_connected(&self.socket_path).await?;
		let json =
			serde_json::to_string(&req).map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;

		let timeout_dur = timeout.unwrap_or(Duration::from_secs(30));
		let resp = self
			.handle_rpc_result(
				rpc.call(
					RequestPayload::LspRequest {
						session_id: self.session_id,
						server_id: ServerId(server.0),
						message: json,
						timeout_ms: Some(timeout_dur.as_millis() as u64),
					},
					timeout_dur + Duration::from_secs(1),
				)
				.await,
				"LspRequest",
			)
			.await?;

		if let ResponsePayload::LspMessage { message, .. } = resp {
			let response: AnyResponse = serde_json::from_str(&message)
				.map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;
			Ok(response)
		} else {
			Err(xeno_lsp::Error::Protocol(
				"unexpected response to LspRequest".into(),
			))
		}
	}

	async fn reply(
		&self,
		server: LanguageServerId,
		resp: Result<JsonValue, ResponseError>,
	) -> Result<()> {
		let rpc = self.ensure_connected(&self.socket_path).await?;
		let server_id = ServerId(server.0);
		let request_id = self
			.pending_server_requests
			.get_mut(&server_id)
			.and_then(|mut queue| queue.pop_front())
			.ok_or_else(|| xeno_lsp::Error::Protocol("no pending request for reply".into()))?;

		let any_resp = match resp {
			Ok(v) => AnyResponse::new_ok(request_id, v),
			Err(e) => AnyResponse::new_err(request_id, e),
		};

		let json = serde_json::to_string(&any_resp)
			.map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;

		self.handle_rpc_result(
			rpc.call(
				RequestPayload::LspReply {
					server_id,
					message: json,
				},
				Duration::from_secs(5),
			)
			.await,
			"LspReply",
		)
		.await?;
		Ok(())
	}
}

struct BrokerClientService {
	tx: mpsc::UnboundedSender<TransportEvent>,
	pending: Arc<DashMap<ServerId, VecDeque<xeno_lsp::RequestId>>>,
	shared_state_tx: mpsc::UnboundedSender<crate::shared_state::SharedStateEvent>,
}

#[derive(Debug)]
struct BrokerTransportShutdown;

impl tower_service::Service<Request> for BrokerClientService {
	type Response = ResponsePayload;
	type Error = ErrorCode;
	type Future = std::pin::Pin<
		Box<
			dyn std::future::Future<Output = std::result::Result<Self::Response, Self::Error>>
				+ Send,
		>,
	>;

	fn poll_ready(
		&mut self,
		_: &mut std::task::Context<'_>,
	) -> std::task::Poll<std::result::Result<(), Self::Error>> {
		std::task::Poll::Ready(Ok(()))
	}

	fn call(&mut self, _req: Request) -> Self::Future {
		Box::pin(async { Err(ErrorCode::UnknownRequest) })
	}
}

impl RpcService<BrokerProtocol> for BrokerClientService {
	type LoopError = std::io::Error;

	fn notify(&mut self, notif: Event) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		match notif {
			Event::Heartbeat => {}
			Event::LspStatus { server_id, status } => {
				let status = match status {
					xeno_broker_proto::types::LspServerStatus::Starting => {
						TransportStatus::Starting
					}
					xeno_broker_proto::types::LspServerStatus::Running => TransportStatus::Running,
					xeno_broker_proto::types::LspServerStatus::Stopped => TransportStatus::Stopped,
					xeno_broker_proto::types::LspServerStatus::Crashed => TransportStatus::Crashed,
				};
				let _ = self.tx.send(TransportEvent::Status {
					server: LanguageServerId(server_id.0),
					status,
				});
			}
			Event::LspMessage { server_id, message } => {
				if let Ok(msg) = serde_json::from_str::<Message>(&message) {
					let _ = self.tx.send(TransportEvent::Message {
						server: LanguageServerId(server_id.0),
						message: msg,
					});
				}
			}
			Event::LspRequest { server_id, message } => {
				if let Ok(msg) = serde_json::from_str::<Message>(&message) {
					if let Message::Request(req) = &msg {
						self.pending
							.entry(server_id)
							.or_default()
							.push_back(req.id.clone());
					}
					let _ = self.tx.send(TransportEvent::Message {
						server: LanguageServerId(server_id.0),
						message: msg,
					});
				}
			}
			Event::LspDiagnostics {
				server_id,
				uri,
				version,
				diagnostics,
				..
			} => {
				if let Ok(diags) = serde_json::from_str::<JsonValue>(&diagnostics) {
					let _ = self.tx.send(TransportEvent::Diagnostics {
						server: LanguageServerId(server_id.0),
						uri,
						version,
						diagnostics: diags,
					});
				}
			}
			Event::SharedDelta {
				uri,
				epoch,
				seq,
				kind,
				tx,
				hash64,
				len_chars,
				..
			} => {
				let _ =
					self.shared_state_tx
						.send(crate::shared_state::SharedStateEvent::RemoteDelta {
							uri,
							epoch,
							seq,
							kind,
							tx,
							hash64,
							len_chars,
						});
			}
			Event::SharedOwnerChanged { snapshot } => {
				let _ = self
					.shared_state_tx
					.send(crate::shared_state::SharedStateEvent::OwnerChanged { snapshot });
			}
			Event::SharedPreferredOwnerChanged { snapshot } => {
				let _ = self.shared_state_tx.send(
					crate::shared_state::SharedStateEvent::PreferredOwnerChanged { snapshot },
				);
			}
			Event::SharedUnlocked { snapshot } => {
				let _ = self
					.shared_state_tx
					.send(crate::shared_state::SharedStateEvent::Unlocked { snapshot });
			}
		}
		ControlFlow::Continue(())
	}

	fn emit(&mut self, event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		if event.is::<BrokerTransportShutdown>() {
			return ControlFlow::Break(Ok(()));
		}
		ControlFlow::Continue(())
	}
}

impl Clone for BrokerRpc {
	fn clone(&self) -> Self {
		Self {
			socket: self.socket.clone(),
		}
	}
}

fn send_shutdown(rpc: &mut Option<BrokerRpc>) {
	if let Some(rpc) = rpc.take() {
		let _ = rpc.socket.emit(BrokerTransportShutdown);
	}
}

impl Drop for BrokerTransport {
	fn drop(&mut self) {
		if Arc::strong_count(&self.rpc) != 1 {
			return;
		}

		if let Ok(mut rpc_lock) = self.rpc.try_lock() {
			send_shutdown(&mut rpc_lock);
			return;
		}

		if let Ok(handle) = tokio::runtime::Handle::try_current() {
			let rpc = self.rpc.clone();
			handle.spawn(async move {
				let mut rpc_lock = rpc.lock().await;
				send_shutdown(&mut rpc_lock);
			});
		}
	}
}
