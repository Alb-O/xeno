//! Broker service implementation.

use std::ops::ControlFlow;
use std::sync::Arc;

use tower_service::Service;
use xeno_broker_proto::BrokerProtocol;
use xeno_broker_proto::types::{
	ErrorCode, Event, Request, RequestPayload, ResponsePayload, SessionId,
};
use xeno_rpc::{AnyEvent, RpcService};

use crate::core::SessionSink;
use crate::runtime::BrokerRuntime;

/// Broker service state and request handlers.
pub struct BrokerService {
	/// Shared broker runtime.
	runtime: Arc<BrokerRuntime>,
	/// Event sink for this connection.
	socket: SessionSink,
	/// Session ID for this connection (once subscribed).
	session_id: Option<SessionId>,
}

impl std::fmt::Debug for BrokerService {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("BrokerService")
			.field("runtime", &"<BrokerRuntime>")
			.field("socket", &"<SessionSink>")
			.field("session_id", &self.session_id)
			.finish()
	}
}

impl BrokerService {
	/// Create a new broker service instance.
	#[must_use]
	pub fn new(runtime: Arc<BrokerRuntime>, socket: SessionSink) -> Self {
		Self {
			runtime,
			socket,
			session_id: None,
		}
	}
}

impl Drop for BrokerService {
	/// Autoritatively cleans up the session when the IPC connection is dropped.
	fn drop(&mut self) {
		if let Some(session_id) = self.session_id {
			let runtime = self.runtime.clone();
			if let Ok(handle) = tokio::runtime::Handle::try_current() {
				handle.spawn(async move {
					runtime.sessions.unregister(session_id).await;
				});
			}
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
		let runtime = self.runtime.clone();
		let socket = self.socket.clone();
		let session_id = self.session_id;

		if let RequestPayload::Subscribe { session_id } = req.payload {
			self.session_id = Some(session_id);
			let rt = runtime.clone();
			// Register inline to avoid race
			return Box::pin(async move {
				rt.sessions.register(session_id, socket).await;
				Ok(ResponsePayload::Subscribed)
			});
		}
		if let RequestPayload::Ping = req.payload {
			return Box::pin(async move { Ok(ResponsePayload::Pong) });
		}

		Box::pin(async move {
			// Enforce auth
			let session_id = session_id.ok_or(ErrorCode::AuthFailed)?;

			match req.payload {
				RequestPayload::Ping => Ok(ResponsePayload::Pong),
				RequestPayload::Subscribe { .. } => Ok(ResponsePayload::Subscribed),
				RequestPayload::LspStart { config } => {
					let server_id = runtime.routing.lsp_start(session_id, config).await?;
					Ok(ResponsePayload::LspStarted { server_id })
				}
				RequestPayload::LspSend {
					session_id: claimed,
					server_id,
					message,
				} => {
					if claimed != session_id {
						return Err(ErrorCode::AuthFailed);
					}
					runtime
						.routing
						.lsp_send_notif(session_id, server_id, message)
						.await?;
					Ok(ResponsePayload::LspSent { server_id })
				}
				RequestPayload::LspRequest {
					session_id: claimed,
					server_id,
					message,
					timeout_ms,
				} => {
					if claimed != session_id {
						return Err(ErrorCode::AuthFailed);
					}
					let req: xeno_lsp::AnyRequest =
						serde_json::from_str(&message).map_err(|_| ErrorCode::InvalidArgs)?;
					let dur = std::time::Duration::from_millis(timeout_ms.unwrap_or(10_000));

					let resp = runtime
						.routing
						.begin_c2s(session_id, server_id, req, dur)
						.await?;
					let json = serde_json::to_string(&resp).map_err(|_| ErrorCode::Internal)?;

					Ok(ResponsePayload::LspMessage {
						server_id,
						message: json,
					})
				}
				RequestPayload::BufferSyncOpen {
					uri,
					text,
					version_hint,
				} => runtime.sync.open(session_id, uri, text, version_hint).await,
				RequestPayload::BufferSyncClose { uri } => {
					runtime.sync.close(session_id, uri).await
				}
				RequestPayload::BufferSyncDelta {
					uri,
					epoch,
					base_seq,
					tx,
				} => {
					runtime
						.sync
						.delta(session_id, uri, epoch, base_seq, tx)
						.await
				}
				RequestPayload::BufferSyncActivity { uri } => {
					runtime.sync.activity(session_id, uri).await
				}
				RequestPayload::BufferSyncTakeOwnership { uri } => {
					runtime.sync.take_ownership(session_id, uri).await
				}
				RequestPayload::BufferSyncReleaseOwnership { uri } => {
					runtime.sync.release_ownership(session_id, uri).await
				}
				RequestPayload::BufferSyncOwnerConfirm {
					uri,
					epoch,
					len_chars,
					hash64,
					allow_mismatch,
				} => {
					runtime
						.sync
						.owner_confirm(session_id, uri, epoch, len_chars, hash64, allow_mismatch)
						.await
				}
				RequestPayload::BufferSyncResync { uri } => {
					runtime.sync.resync(session_id, uri).await
				}
				RequestPayload::KnowledgeSearch { query, limit } => {
					let hits = runtime.knowledge.search(&query, limit).await?;
					Ok(ResponsePayload::KnowledgeSearchResults { hits })
				}
				RequestPayload::LspReply { server_id, message } => {
					let resp: xeno_lsp::AnyResponse =
						serde_json::from_str(&message).map_err(|_| ErrorCode::InvalidArgs)?;

					// We need result type
					let result = if let Some(error) = resp.error {
						Err(error)
					} else {
						Ok(resp.result.unwrap_or(serde_json::Value::Null))
					};
					let request_id = resp.id;

					if runtime
						.routing
						.complete_s2c(session_id, server_id, request_id.clone(), result)
						.await
					{
						Ok(ResponsePayload::LspSent { server_id })
					} else {
						Err(ErrorCode::RequestNotFound)
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
