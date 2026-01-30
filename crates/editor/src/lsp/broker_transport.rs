//! Broker-based LSP transport implementation.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use xeno_broker_proto::BrokerProtocol;
use xeno_broker_proto::types::{
	ErrorCode, Event, IpcFrame, Request, RequestId, RequestPayload, Response, ResponsePayload,
	ServerId, SessionId,
};
use xeno_lsp::client::LanguageServerId;
use xeno_lsp::client::transport::{LspTransport, StartedServer, TransportEvent, TransportStatus};
use xeno_lsp::{AnyNotification, AnyRequest, AnyResponse, JsonValue, Message, ResponseError};
use xeno_rpc::{AnyEvent, MainLoop, MainLoopEvent, PeerSocket, RpcService};

/// Transport that communicates with a Xeno broker over a Unix socket.
pub struct BrokerTransport {
	session_id: SessionId,
	events_tx: mpsc::UnboundedSender<TransportEvent>,
	events_rx: std::sync::Mutex<Option<mpsc::UnboundedReceiver<TransportEvent>>>,
	rpc: tokio::sync::Mutex<Option<BrokerRpc>>,
	pending_server_requests: DashMap<ServerId, xeno_lsp::RequestId>,
}

struct BrokerRpc {
	socket: PeerSocket<IpcFrame, Request, Response>,
}

impl BrokerTransport {
	/// Create a new broker transport.
	pub fn new() -> Arc<Self> {
		let (tx, rx) = mpsc::unbounded_channel();
		let session_id = SessionId(std::process::id() as u64);
		Arc::new(Self {
			session_id,
			events_tx: tx,
			events_rx: std::sync::Mutex::new(Some(rx)),
			rpc: tokio::sync::Mutex::new(None),
			pending_server_requests: DashMap::new(),
		})
	}

	/// Ensure we are connected to the broker.
	async fn ensure_connected(
		&self,
		socket_path: &std::path::Path,
	) -> Result<BrokerRpc, xeno_lsp::Error> {
		let mut rpc_lock = self.rpc.lock().await;
		if let Some(rpc) = &*rpc_lock {
			return Ok(BrokerRpc {
				socket: rpc.socket.clone(),
			});
		}

		let stream = UnixStream::connect(socket_path)
			.await
			.map_err(xeno_lsp::Error::Io)?;
		let (r, w) = stream.into_split();
		let reader = tokio::io::BufReader::new(r);

		let protocol = BrokerProtocol::new();
		let id_gen = xeno_rpc::CounterIdGen::new();

		let events_tx = self.events_tx.clone();
		let pending = self.pending_server_requests.clone();
		let (main_loop, socket) = MainLoop::new(
			move |_| BrokerClientService {
				tx: events_tx.clone(),
				pending: pending.clone(),
			},
			protocol,
			id_gen,
		);

		let events_tx = self.events_tx.clone();
		tokio::spawn(async move {
			let _ = main_loop.run(reader, w).await;
			let _ = events_tx.send(TransportEvent::Disconnected);
		});

		let rpc = BrokerRpc {
			socket: socket.clone(),
		};

		// Subscribe
		rpc.call(
			RequestPayload::Subscribe {
				session_id: self.session_id,
			},
			Duration::from_secs(5),
		)
		.await
		.map_err(|e| xeno_lsp::Error::Protocol(format!("Subscribe failed: {:?}", e)))?;

		*rpc_lock = Some(BrokerRpc {
			socket: socket.clone(),
		});
		Ok(rpc)
	}
}

impl BrokerRpc {
	async fn call(
		&self,
		payload: RequestPayload,
		timeout_dur: Duration,
	) -> Result<ResponsePayload, ErrorCode> {
		let req = Request {
			id: RequestId(0),
			payload,
		};
		let (tx, rx) = oneshot::channel();
		let _ = self.socket.send(MainLoopEvent::OutgoingRequest(req, tx));

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

	async fn start(&self, cfg: xeno_lsp::ServerConfig) -> xeno_lsp::Result<StartedServer> {
		let socket_path = std::env::temp_dir().join("xeno-broker.sock");
		let rpc = self.ensure_connected(&socket_path).await?;

		let broker_cfg = xeno_broker_proto::types::LspServerConfig {
			command: cfg.command,
			args: cfg.args,
			env: cfg.env.into_iter().collect(),
			cwd: Some(cfg.root_path.to_string_lossy().to_string()),
		};

		let resp = rpc
			.call(
				RequestPayload::LspStart { config: broker_cfg },
				Duration::from_secs(10),
			)
			.await
			.map_err(|e| xeno_lsp::Error::Protocol(format!("LspStart failed: {:?}", e)))?;

		if let ResponsePayload::LspStarted { server_id } = resp {
			Ok(StartedServer {
				id: LanguageServerId(server_id.0),
			})
		} else {
			Err(xeno_lsp::Error::Protocol(
				"Unexpected response to LspStart".into(),
			))
		}
	}

	async fn notify(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> xeno_lsp::Result<()> {
		let rpc = self
			.ensure_connected(&std::env::temp_dir().join("xeno-broker.sock"))
			.await?;
		let msg = Message::Notification(notif);
		let json =
			serde_json::to_string(&msg).map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;

		rpc.call(
			RequestPayload::LspSend {
				server_id: ServerId(server.0),
				message: json,
			},
			Duration::from_secs(5),
		)
		.await
		.map_err(|e| xeno_lsp::Error::Protocol(format!("LspSend failed: {:?}", e)))?;

		Ok(())
	}

	async fn notify_with_barrier(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> xeno_lsp::Result<oneshot::Receiver<()>> {
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
	) -> xeno_lsp::Result<AnyResponse> {
		let rpc = self
			.ensure_connected(&std::env::temp_dir().join("xeno-broker.sock"))
			.await?;
		let json =
			serde_json::to_string(&req).map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;

		let timeout_dur = timeout.unwrap_or(Duration::from_secs(30));
		let resp = rpc
			.call(
				RequestPayload::LspRequest {
					server_id: ServerId(server.0),
					message: json,
					timeout_ms: Some(timeout_dur.as_millis() as u64),
				},
				timeout_dur + Duration::from_secs(1),
			)
			.await
			.map_err(|e| xeno_lsp::Error::Protocol(format!("LspRequest failed: {:?}", e)))?;

		if let ResponsePayload::LspMessage { message, .. } = resp {
			let response: AnyResponse = serde_json::from_str(&message)
				.map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;
			Ok(response)
		} else {
			Err(xeno_lsp::Error::Protocol(
				"Unexpected response to LspRequest".into(),
			))
		}
	}

	async fn reply(
		&self,
		server: LanguageServerId,
		resp: Result<JsonValue, ResponseError>,
	) -> xeno_lsp::Result<()> {
		let rpc = self
			.ensure_connected(&std::env::temp_dir().join("xeno-broker.sock"))
			.await?;
		let server_id = ServerId(server.0);
		let request_id = self
			.pending_server_requests
			.remove(&server_id)
			.ok_or_else(|| xeno_lsp::Error::Protocol("No pending request for reply".into()))?
			.1;

		let any_resp = match resp {
			Ok(v) => AnyResponse::new_ok(request_id, v),
			Err(e) => AnyResponse::new_err(request_id, e),
		};

		let json = serde_json::to_string(&any_resp)
			.map_err(|e| xeno_lsp::Error::Protocol(e.to_string()))?;

		rpc.call(
			RequestPayload::LspReply {
				server_id,
				message: json,
			},
			Duration::from_secs(5),
		)
		.await
		.map_err(|e| xeno_lsp::Error::Protocol(format!("LspReply failed: {:?}", e)))?;

		Ok(())
	}
}

struct BrokerClientService {
	tx: mpsc::UnboundedSender<TransportEvent>,
	pending: DashMap<ServerId, xeno_lsp::RequestId>,
}

impl tower_service::Service<Request> for BrokerClientService {
	type Response = ResponsePayload;
	type Error = ErrorCode;
	type Future = std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
	>;

	fn poll_ready(
		&mut self,
		_: &mut std::task::Context<'_>,
	) -> std::task::Poll<Result<(), Self::Error>> {
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
						self.pending.insert(server_id, req.id.clone());
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
		}
		ControlFlow::Continue(())
	}

	fn emit(&mut self, _event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
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
