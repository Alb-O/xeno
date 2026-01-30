//! Broker service implementation.

use std::ops::ControlFlow;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::io::BufReader;
use tower_service::Service;
use xeno_broker_proto::types::{
	ErrorCode, Event, LspServerConfig, LspServerStatus, Request, RequestPayload, ResponsePayload,
	ServerId, SessionId,
};
use xeno_rpc::{AnyEvent, RpcService};

use crate::core::{BrokerCore, LspInstance, SessionSink};
use crate::lsp::LspProxyService;
use crate::protocol::BrokerProtocol;

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
		let server_id = core.next_server_id();

		let mut child = tokio::process::Command::new(config.command)
			.args(config.args)
			.envs(config.env)
			.current_dir(config.cwd.unwrap_or_default())
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
			move |_| LspProxyService::new(core_clone.clone(), session_id, server_id),
			protocol,
			id_gen,
		);

		let instance = LspInstance {
			owner: session_id,
			server_id,
			lsp_tx: lsp_socket,
			child,
			status: Mutex::new(LspServerStatus::Starting),
		};
		core.register_server(server_id, instance);

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

					core.on_editor_message(server_id, &lsp_msg);

					let _ = lsp_tx.send(xeno_rpc::MainLoopEvent::Outgoing(lsp_msg));

					Ok(ResponsePayload::LspSent { server_id })
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
