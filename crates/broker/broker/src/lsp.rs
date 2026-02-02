//! LSP proxy service for forwarding messages between LSP servers and editor sessions.

use std::ops::ControlFlow;
use std::time::Duration;

use tokio::time::timeout;
use tower_service::Service;
use xeno_broker_proto::types::ServerId;
use xeno_lsp::protocol::JsonRpcProtocol;
use xeno_lsp::{AnyNotification, AnyRequest, ErrorCode, Message, ResponseError};
use xeno_rpc::{AnyEvent, RpcService};

use crate::services::routing::RoutingHandle;

/// Proxies messages from an LSP server to the broker routing service.
///
/// This service acts as a Language Client on the server's stdio. It receives
/// responses and notifications from the LSP server and forwards them to the
/// `RoutingService` for dispatch to editor sessions.
#[derive(Debug)]
pub struct LspProxyService {
	routing: RoutingHandle,
	server_id: ServerId,
}

impl LspProxyService {
	/// Creates a new LSP proxy service instance.
	#[must_use]
	pub fn new(routing: RoutingHandle, server_id: ServerId) -> Self {
		Self { routing, server_id }
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

	/// Handles an inbound request from the LSP server.
	///
	/// Serializes the request and forwards it to the `RoutingService`. If the
	/// request times out or the broker shuts down, the pending request is
	/// cancelled to avoid leaks.
	fn call(&mut self, req: AnyRequest) -> Self::Future {
		let routing = self.routing.clone();
		let server_id = self.server_id;
		let request_id = req.id.clone();

		let json = match serde_json::to_string(&Message::Request(req)) {
			Ok(j) => j,
			Err(_) => {
				return Box::pin(async {
					Err(ResponseError::new(
						ErrorCode::PARSE_ERROR,
						"Failed to serialize request",
					))
				});
			}
		};

		Box::pin(async move {
			let (tx, rx) = tokio::sync::oneshot::channel();

			routing
				.begin_s2c(server_id, request_id.clone(), json, tx)
				.await?;

			match timeout(Duration::from_secs(30), rx).await {
				Ok(Ok(result)) => result,
				Ok(Err(_)) => Err(ResponseError::new(
					ErrorCode::INTERNAL_ERROR,
					"Broker internal error: reply channel closed",
				)),
				Err(_) => {
					routing.cancel_s2c(server_id, request_id).await;
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

	/// Handles an inbound notification from the LSP server.
	fn notify(
		&mut self,
		notif: AnyNotification,
	) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		let json = match serde_json::to_string(&Message::Notification(notif)) {
			Ok(json) => json,
			Err(e) => {
				tracing::error!(error = %e, "Failed to serialize LSP message for proxy");
				return ControlFlow::Continue(());
			}
		};

		let routing = self.routing.clone();
		let server_id = self.server_id;

		tokio::spawn(async move {
			routing.server_notif(server_id, json).await;
		});

		ControlFlow::Continue(())
	}

	fn emit(&mut self, _event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		ControlFlow::Continue(())
	}
}
