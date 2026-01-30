//! LSP proxy service for forwarding messages between LSP servers and editor sessions.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;
use tower_service::Service;
use xeno_broker_proto::types::{Event, IpcFrame, LspServerStatus, ServerId, SessionId};
use xeno_lsp::protocol::JsonRpcProtocol;
use xeno_lsp::{AnyNotification, AnyRequest, ErrorCode, Message, ResponseError};
use xeno_rpc::{AnyEvent, RpcService};

use crate::core::BrokerCore;

/// Service that proxies messages from an LSP server to the broker core.
///
/// This service acts as a Language Client on the server's stdio. It receives
/// responses and notifications from the LSP server, serializes them to JSON,
/// and forwards them to the owning editor session as IPC events.
#[derive(Debug)]
pub struct LspProxyService {
	/// Shared broker core for event fan-out.
	core: Arc<BrokerCore>,
	/// Session that owns this LSP instance.
	owner: SessionId,
	/// Server ID assigned to this instance.
	server_id: ServerId,
}

impl LspProxyService {
	/// Create a new LSP proxy service instance.
	#[must_use]
	pub fn new(core: Arc<BrokerCore>, owner: SessionId, server_id: ServerId) -> Self {
		Self {
			core,
			owner,
			server_id,
		}
	}

	/// Forward an inbound LSP message to the owner session.
	///
	/// If the message is a `publishDiagnostics` notification, it also emits
	/// a structured `Event::LspDiagnostics` to the session.
	fn forward(&self, msg: Message) {
		// Transition status to Running on receipt of any message from the server.
		self.core
			.set_server_status(self.server_id, LspServerStatus::Running);

		let json = match serde_json::to_string(&msg) {
			Ok(json) => json,
			Err(e) => {
				tracing::error!(error = %e, "Failed to serialize LSP message for proxy");
				return;
			}
		};

		self.core.send_event(
			self.owner,
			IpcFrame::Event(Event::LspMessage {
				server_id: self.server_id,
				message: json,
			}),
		);

		// Extract structured diagnostics if applicable.
		if let Message::Notification(notif) = &msg
			&& notif.method == "textDocument/publishDiagnostics"
			&& let Some(uri) = notif.params.get("uri").and_then(|u| u.as_str())
			&& let Some((doc_id, version)) = self.core.get_doc_by_uri(uri)
		{
			let diagnostics = notif
				.params
				.get("diagnostics")
				.map(ToString::to_string)
				.unwrap_or_else(|| "[]".to_string());
			self.core.send_event(
				self.owner,
				IpcFrame::Event(Event::LspDiagnostics {
					doc_id,
					version,
					diagnostics,
				}),
			);
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

		// Forward request to editor as an async event
		self.forward(Message::Request(req));

		// Register a oneshot and wait for the editor to reply via LspReply.
		let (tx, rx) = tokio::sync::oneshot::channel();
		core.register_client_request(server_id, request_id, tx);

		Box::pin(async move {
			// Wait for reply from editor (with 30s timeout for client requests)
			match timeout(Duration::from_secs(30), rx).await {
				Ok(Ok(result)) => result,
				Ok(Err(_)) => Err(ResponseError::new(
					ErrorCode::INTERNAL_ERROR,
					"Broker internal error: reply channel closed",
				)),
				Err(_) => Err(ResponseError::new(
					ErrorCode::REQUEST_CANCELLED,
					"Broker timeout waiting for editor reply",
				)),
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
