//! Request routing and pending request management.
//!
//! Handles server-to-client and client-to-server request lifecycle,
//! including wire ID allocation and pending map management.

use tokio::sync::oneshot;
use xeno_broker_proto::types::{ServerId, SessionId};

use super::{BrokerCore, LspReplyResult, PendingC2sReq, PendingS2cReq};

impl BrokerCore {
	/// Register a pending server-to-editor request.
	///
	/// Returns the [`SessionId`] of the leader elected to respond.
	pub fn register_client_request(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		tx: oneshot::Sender<LspReplyResult>,
	) -> Option<SessionId> {
		let mut state = self.state.lock().unwrap();
		let server = state.servers.get(&server_id)?;
		if server.attached.is_empty() {
			return None;
		}
		let leader = server.leader;

		state.pending_s2c.insert(
			(server_id, request_id),
			PendingS2cReq {
				responder: leader,
				tx,
			},
		);

		Some(leader)
	}

	/// Complete a pending server-to-editor request with a reply.
	///
	/// Returns true if the request was successfully completed.
	pub fn complete_client_request(
		&self,
		session_id: SessionId,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: LspReplyResult,
	) -> bool {
		let mut state = self.state.lock().unwrap();
		let Some(req) = state.pending_s2c.get(&(server_id, request_id.clone())) else {
			return false;
		};
		if req.responder != session_id {
			return false;
		}

		if let Some(req) = state.pending_s2c.remove(&(server_id, request_id)) {
			let _ = req.tx.send(result);
			true
		} else {
			false
		}
	}

	/// Cancel a pending server-to-editor request with a standard error response.
	pub fn cancel_client_request(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
	) -> bool {
		let mut state = self.state.lock().unwrap();
		if let Some(req) = state.pending_s2c.remove(&(server_id, request_id)) {
			let _ = req.tx.send(Err(xeno_lsp::ResponseError::new(
				xeno_lsp::ErrorCode::REQUEST_CANCELLED,
				"request cancelled by broker",
			)));
			true
		} else {
			false
		}
	}

	/// Cancel all pending server-to-client requests for a given session.
	pub fn cancel_all_client_requests_for_session(&self, session_id: SessionId) {
		let to_cancel: Vec<(ServerId, xeno_lsp::RequestId)> = {
			let state = self.state.lock().unwrap();
			state
				.pending_s2c
				.iter()
				.filter(|(_, req)| req.responder == session_id)
				.map(|(key, _)| key.clone())
				.collect()
		};
		for (server_id, request_id) in to_cancel {
			self.cancel_client_request(server_id, request_id);
		}
	}

	/// Cancel all pending server-to-client requests for a given server.
	pub fn cancel_all_client_requests_for_server(&self, server_id: ServerId) {
		let to_cancel: Vec<xeno_lsp::RequestId> = {
			let state = self.state.lock().unwrap();
			state
				.pending_s2c
				.iter()
				.filter(|((sid, _), _)| *sid == server_id)
				.map(|((_, rid), _)| rid.clone())
				.collect()
		};
		for request_id in to_cancel {
			self.cancel_client_request(server_id, request_id);
		}
	}

	/// Allocate a unique wire request ID for a server connection.
	///
	/// Uses string IDs ("b:{server}:{seq}") to prevent numeric overflow.
	pub fn alloc_wire_request_id(&self, server_id: ServerId) -> Option<xeno_lsp::RequestId> {
		let mut state = self.state.lock().unwrap();
		let server = state.servers.get_mut(&server_id)?;
		let wire_num = server.next_wire_req_id;
		server.next_wire_req_id += 1;
		Some(xeno_lsp::RequestId::String(format!(
			"b:{}:{}",
			server_id.0, wire_num
		)))
	}

	/// Register a pending client-to-server request mapping.
	///
	/// # Arguments
	/// * `server_id` - Target LSP server.
	/// * `wire_id` - ID used on the wire to the server.
	/// * `origin_session` - Editor session that initiated the request.
	/// * `origin_id` - Original request ID from the editor.
	pub fn register_c2s_pending(
		&self,
		server_id: ServerId,
		wire_id: xeno_lsp::RequestId,
		origin_session: SessionId,
		origin_id: xeno_lsp::RequestId,
	) {
		let mut state = self.state.lock().unwrap();
		state.pending_c2s.insert(
			(server_id, wire_id),
			PendingC2sReq {
				origin_session,
				origin_id,
			},
		);
	}

	/// Remove and return the pending client-to-server mapping for a server and wire ID.
	pub fn take_c2s_pending(
		&self,
		server_id: ServerId,
		wire_id: &xeno_lsp::RequestId,
	) -> Option<PendingC2sReq> {
		let mut state = self.state.lock().unwrap();
		state.pending_c2s.remove(&(server_id, wire_id.clone()))
	}

	/// Cancel a pending client-to-server request and return the origin information.
	pub fn cancel_c2s_pending(
		&self,
		server_id: ServerId,
		wire_id: &xeno_lsp::RequestId,
	) -> Option<PendingC2sReq> {
		self.take_c2s_pending(server_id, wire_id)
	}
}
