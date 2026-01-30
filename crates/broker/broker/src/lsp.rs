//! LSP proxy service for forwarding messages between LSP servers and editor sessions.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;
use tower_service::Service;
use xeno_broker_proto::types::{Event, LspServerStatus, ServerId};
use xeno_lsp::protocol::JsonRpcProtocol;
use xeno_lsp::{AnyNotification, AnyRequest, ErrorCode, Message, ResponseError};
use xeno_rpc::{AnyEvent, RpcService};

use crate::core::BrokerCore;

/// Service that proxies messages from an LSP server to the broker core.
///
/// This service acts as a Language Client on the server's stdio. It receives
/// responses and notifications from the LSP server, serializes them to JSON,
/// and forwards them to the attached editor sessions as IPC events.
#[derive(Debug)]
pub struct LspProxyService {
	/// Shared broker core for event fan-out.
	core: Arc<BrokerCore>,
	/// Server ID assigned to this instance.
	server_id: ServerId,
}

impl LspProxyService {
	/// Create a new LSP proxy service instance.
	#[must_use]
	pub fn new(core: Arc<BrokerCore>, server_id: ServerId) -> Self {
		Self { core, server_id }
	}

	/// Forward an inbound LSP message to the attached session(s).
	///
	/// Transitions server status to [`LspServerStatus::Running`] upon receipt.
	/// Notifications are broadcast to all sessions, while requests are routed
	/// only to the currently elected leader.
	fn forward(&self, msg: Message) {
		self.core
			.set_server_status(self.server_id, LspServerStatus::Running);

		let json = match serde_json::to_string(&msg) {
			Ok(json) => json,
			Err(e) => {
				tracing::error!(error = %e, "Failed to serialize LSP message for proxy");
				return;
			}
		};

		match msg {
			Message::Request(ref req) => {
				tracing::trace!(?self.server_id, method = %req.method, "Forwarding server request to leader");
				self.core.send_to_leader(
					self.server_id,
					Event::LspRequest {
						server_id: self.server_id,
						message: json,
					},
				);
			}
			Message::Notification(ref notif) => {
				tracing::trace!(?self.server_id, method = %notif.method, "Broadcasting server notification");
				self.core.broadcast_to_server(
					self.server_id,
					Event::LspMessage {
						server_id: self.server_id,
						message: json,
					},
				);

				if notif.method == "textDocument/publishDiagnostics"
					&& let Some(uri) = notif.params.get("uri").and_then(|u| u.as_str())
					&& let Some((doc_id, version)) = self.core.get_doc_by_uri(self.server_id, uri)
				{
					tracing::debug!(?self.server_id, %uri, "Broadcasting structured diagnostics");
					let diagnostics = notif
						.params
						.get("diagnostics")
						.map(ToString::to_string)
						.unwrap_or_else(|| "[]".to_string());
					self.core.broadcast_to_server(
						self.server_id,
						Event::LspDiagnostics {
							server_id: self.server_id,
							doc_id,
							uri: uri.to_string(),
							version,
							diagnostics,
						},
					);
				}
			}
			Message::Response(_) => {}
		}
	}
}

impl Service<AnyRequest> for LspProxyService {
	type Response = serde_json::Value;
	type Error = ResponseError;
	type Future = std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
	>;

	fn poll_ready(
		&mut self,
		_cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Result<(), Self::Error>> {
		std::task::Poll::Ready(Ok(()))
	}

	fn call(&mut self, req: AnyRequest) -> Self::Future {
		let core = self.core.clone();
		let server_id = self.server_id;
		let request_id = req.id.clone();

		// Register a oneshot FIRST (before forwarding) to avoid race condition
		// where leader replies before we register the pending request.
		let (tx, rx) = tokio::sync::oneshot::channel();
		if core
			.register_client_request(server_id, request_id.clone(), tx)
			.is_none()
		{
			return Box::pin(async {
				Err(ResponseError::new(
					ErrorCode::METHOD_NOT_FOUND,
					"No leader session available for request",
				))
			});
		}

		// Now forward request to leader as an async event
		self.forward(Message::Request(req));

		Box::pin(async move {
			// Wait for reply from editor (with 30s timeout for client requests)
			match timeout(Duration::from_secs(30), rx).await {
				Ok(Ok(result)) => result,
				Ok(Err(_)) => Err(ResponseError::new(
					ErrorCode::INTERNAL_ERROR,
					"Broker internal error: reply channel closed",
				)),
				Err(_) => {
					// Timeout: cancel the pending request to prevent memory leak
					core.cancel_client_request(server_id, request_id);
					Err(ResponseError::new(
						ErrorCode::REQUEST_CANCELLED,
						"Broker timeout waiting for editor reply",
					))
				}
			}
		})
	}
}

impl RpcService<JsonRpcProtocol> for LspProxyService {
	type LoopError = xeno_lsp::Error;

	fn notify(
		&mut self,
		notif: AnyNotification,
	) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		self.forward(Message::Notification(notif));
		ControlFlow::Continue(())
	}

	fn emit(&mut self, _event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		ControlFlow::Continue(())
	}
}
