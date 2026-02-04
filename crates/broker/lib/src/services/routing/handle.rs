use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use xeno_broker_proto::types::{ErrorCode, LspServerConfig, ServerId, SessionId};
use xeno_lsp::{AnyRequest, AnyResponse};

use super::commands::RoutingCmd;

/// Handle for communicating with the `RoutingService`.
#[derive(Clone, Debug)]
pub struct RoutingHandle {
	tx: mpsc::Sender<RoutingCmd>,
}

impl RoutingHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<RoutingCmd>) -> Self {
		Self { tx }
	}

	/// Starts an LSP server.
	pub async fn lsp_start(
		&self,
		sid: SessionId,
		config: LspServerConfig,
	) -> Result<ServerId, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::LspStart { sid, config, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Registers and transmits a server-to-client request atomically.
	pub async fn begin_s2c(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		json: String,
		tx: oneshot::Sender<crate::core::LspReplyResult>,
	) -> Result<(), xeno_lsp::ResponseError> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::BeginS2c {
				server_id,
				request_id,
				json,
				tx,
				reply,
			})
			.await
			.map_err(|_| {
				xeno_lsp::ResponseError::new(
					xeno_lsp::ErrorCode::INTERNAL_ERROR,
					"broker shut down",
				)
			})?;
		rx.await.map_err(|_| {
			xeno_lsp::ResponseError::new(xeno_lsp::ErrorCode::INTERNAL_ERROR, "broker shut down")
		})?
	}

	/// Delivers a reply to a pending server-to-client request.
	pub async fn complete_s2c(
		&self,
		sid: SessionId,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: crate::core::LspReplyResult,
	) -> bool {
		let (reply, rx) = oneshot::channel();
		if self
			.tx
			.send(RoutingCmd::CompleteS2c {
				sid,
				server_id,
				request_id,
				result,
				reply,
			})
			.await
			.is_err()
		{
			return false;
		}
		rx.await.unwrap_or(false)
	}

	/// Cancels a server-to-client request.
	pub async fn cancel_s2c(&self, server_id: ServerId, request_id: xeno_lsp::RequestId) {
		let _ = self
			.tx
			.send(RoutingCmd::CancelS2c {
				server_id,
				request_id,
			})
			.await;
	}

	/// Initiates an editor-to-server request.
	pub async fn begin_c2s(
		&self,
		sid: SessionId,
		server_id: ServerId,
		req: AnyRequest,
		timeout: Duration,
	) -> Result<AnyResponse, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::BeginC2s {
				sid,
				server_id,
				req,
				timeout,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Authoritatively cleans up a lost session.
	pub async fn session_lost(&self, sid: SessionId) {
		let _ = self.tx.send(RoutingCmd::SessionLost { sid }).await;
	}

	/// Delivers an editor notification.
	pub async fn lsp_send_notif(
		&self,
		sid: SessionId,
		server_id: ServerId,
		message: String,
	) -> Result<(), ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::LspSendNotif {
				sid,
				server_id,
				message,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Delivers a server notification.
	pub async fn server_notif(&self, server_id: ServerId, message: String) {
		let _ = self
			.tx
			.send(RoutingCmd::ServerNotif { server_id, message })
			.await;
	}

	/// Registers a document open from buffer sync.
	pub async fn lsp_doc_open(&self, uri: String, text: String) {
		let _ = self.tx.send(RoutingCmd::LspDocOpen { uri, text }).await;
	}

	/// Registers a document update from buffer sync.
	pub async fn lsp_doc_update(&self, uri: String, text: String) {
		let _ = self.tx.send(RoutingCmd::LspDocUpdate { uri, text }).await;
	}

	/// Registers a document close from buffer sync.
	pub async fn lsp_doc_close(&self, uri: String) {
		let _ = self.tx.send(RoutingCmd::LspDocClose { uri }).await;
	}

	/// Delivers a process exit signal.
	pub async fn server_exited(&self, server_id: ServerId, crashed: bool) {
		let _ = self
			.tx
			.send(RoutingCmd::ServerExited { server_id, crashed })
			.await;
	}

	/// Shutdown all managed servers.
	pub async fn terminate_all(&self) {
		let _ = self.tx.send(RoutingCmd::TerminateAll).await;
	}
}
