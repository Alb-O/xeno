//! Broker service implementation.

use std::ops::ControlFlow;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::BufReader;
use tokio::time::timeout;
use tower_service::Service;
use xeno_broker_proto::BrokerProtocol;
use xeno_broker_proto::types::{
	ErrorCode, Event, LspServerConfig, LspServerStatus, Request, RequestPayload, ResponsePayload,
	ServerId, SessionId,
};
use xeno_rpc::{AnyEvent, RpcService};

use crate::core::{BrokerCore, LspInstance, SessionSink};
use crate::lsp::LspProxyService;

/// Broker service state and request handlers.
///
/// Each IPC connection to the broker is handled by an instance of this service.
/// It routes editor requests to the shared [`BrokerCore`] or specific LSP servers.
#[derive(Debug)]
pub struct BrokerService {
	/// Shared broker core.
	core: Arc<BrokerCore>,
	/// Event sink for this connection.
	socket: SessionSink,
	/// Session ID for this connection (once subscribed).
	session_id: Option<SessionId>,
}

impl BrokerService {
	/// Create a new broker service instance.
	#[must_use]
	pub fn new(core: Arc<BrokerCore>, socket: SessionSink) -> Self {
		Self {
			core,
			socket,
			session_id: None,
		}
	}

	/// Starts a new LSP server instance and returns its globally unique ID.
	///
	/// This spawns the server process, initializes the RPC main loop over its
	/// stdio, and registers it with the core.
	async fn lsp_start(
		core: Arc<BrokerCore>,
		session_id: SessionId,
		config: LspServerConfig,
	) -> Result<ServerId, ErrorCode> {
		// Check for existing server for this project
		if let Some(server_id) = core.find_server_for_project(&config)
			&& core.attach_session(server_id, session_id)
		{
			return Ok(server_id);
		}

		let server_id = core.next_server_id();

		let mut child = tokio::process::Command::new(&config.command)
			.args(&config.args)
			.envs(config.env.iter().cloned())
			.current_dir(config.cwd.as_deref().unwrap_or_default())
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::inherit())
			.spawn()
			.map_err(|e| {
				tracing::error!(error = %e, "Failed to spawn LSP server");
				ErrorCode::Internal
			})?;

		let stdin = child.stdin.take().ok_or(ErrorCode::Internal)?;
		let stdout = child.stdout.take().ok_or(ErrorCode::Internal)?;

		let protocol = xeno_lsp::protocol::JsonRpcProtocol::new();
		let id_gen = xeno_rpc::CounterIdGen::new();

		let core_clone = core.clone();
		let (lsp_loop, lsp_socket) = xeno_rpc::MainLoop::new(
			move |_| LspProxyService::new(core_clone.clone(), server_id),
			protocol,
			id_gen,
		);

		let instance = LspInstance {
			lsp_tx: lsp_socket,
			child,
			status: Mutex::new(LspServerStatus::Starting),
		};
		core.register_server(server_id, instance, &config, session_id);

		let core_clone = core.clone();
		tokio::spawn(async move {
			let reader = BufReader::new(stdout);
			let _ = lsp_loop.run(reader, stdin).await;

			core_clone.unregister_server(server_id);
			core_clone.set_server_status(server_id, LspServerStatus::Stopped);
		});

		core.set_server_status(server_id, LspServerStatus::Starting);

		Ok(server_id)
	}
}

impl Drop for BrokerService {
	fn drop(&mut self) {
		if let Some(session_id) = self.session_id {
			self.core.unregister_session(session_id);
		}
	}
}

impl Service<Request> for BrokerService {
	type Response = ResponsePayload;
	type Error = ErrorCode;
	type Future = std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
	>;

	fn poll_ready(
		&mut self,
		_cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Result<(), Self::Error>> {
		std::task::Poll::Ready(Ok(()))
	}

	fn call(&mut self, req: Request) -> Self::Future {
		let core = self.core.clone();
		let socket = self.socket.clone();
		let session_id = self.session_id;

		// Session registration on subscription.
		if let RequestPayload::Subscribe { session_id } = req.payload {
			self.session_id = Some(session_id);
			core.register_session(session_id, socket);
		}

		Box::pin(async move {
			match req.payload {
				RequestPayload::Ping => Ok(ResponsePayload::Pong),
				RequestPayload::Subscribe { .. } => Ok(ResponsePayload::Subscribed),
				RequestPayload::LspStart { config } => {
					let session_id = session_id.ok_or(ErrorCode::AuthFailed)?;
					let server_id = Self::lsp_start(core, session_id, config).await?;
					Ok(ResponsePayload::LspStarted { server_id })
				}
				RequestPayload::LspSend { server_id, message } => {
					let lsp_tx = core
						.get_server_tx(server_id)
						.ok_or(ErrorCode::ServerNotFound)?;

					let lsp_msg: xeno_lsp::Message =
						serde_json::from_str(&message).map_err(|_| ErrorCode::InvalidArgs)?;

					// Enforce notifications only for LspSend
					if matches!(lsp_msg, xeno_lsp::Message::Request(_)) {
						return Err(ErrorCode::InvalidArgs);
					}

					core.on_editor_message(server_id, &lsp_msg);

					let _ = lsp_tx.send(xeno_rpc::MainLoopEvent::Outgoing(lsp_msg));

					Ok(ResponsePayload::LspSent { server_id })
				}
				RequestPayload::LspRequest {
					server_id,
					message,
					timeout_ms,
				} => {
					let lsp_tx = core
						.get_server_tx(server_id)
						.ok_or(ErrorCode::ServerNotFound)?;

					let req: xeno_lsp::AnyRequest =
						serde_json::from_str(&message).map_err(|_| ErrorCode::InvalidArgs)?;

					let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
					let _ = lsp_tx.send(xeno_rpc::MainLoopEvent::OutgoingRequest(req, resp_tx));

					let dur = Duration::from_millis(timeout_ms.unwrap_or(10_000));
					let resp = timeout(dur, resp_rx)
						.await
						.map_err(|_| ErrorCode::Timeout)?
						.map_err(|_| ErrorCode::Internal)?;

					let json = serde_json::to_string(&resp).map_err(|_| ErrorCode::Internal)?;
					Ok(ResponsePayload::LspMessage {
						server_id,
						message: json,
					})
				}
				RequestPayload::LspReply { server_id, message } => {
					let session_id = session_id.ok_or(ErrorCode::AuthFailed)?;
					let resp: xeno_lsp::AnyResponse =
						serde_json::from_str(&message).map_err(|_| ErrorCode::InvalidArgs)?;

					let request_id = resp.id.clone();
					let result = if let Some(error) = resp.error {
						Err(error)
					} else {
						Ok(resp.result.unwrap_or(serde_json::Value::Null))
					};

					if core.complete_client_request(session_id, server_id, request_id, result) {
						Ok(ResponsePayload::LspSent { server_id })
					} else {
						Err(ErrorCode::Internal)
					}
				}
			}
		})
	}
}

impl RpcService<BrokerProtocol> for BrokerService {
	type LoopError = std::io::Error;

	fn notify(&mut self, _notif: Event) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		ControlFlow::Continue(())
	}

	fn emit(&mut self, _event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		ControlFlow::Continue(())
	}
}
