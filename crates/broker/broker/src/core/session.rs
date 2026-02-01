//! Session lifecycle management.
//!
//! Methods for registering, unregistering, and cleaning up editor sessions.

use std::sync::Arc;

use xeno_broker_proto::types::SessionId;

use super::{BrokerCore, ServerId};

impl BrokerCore {
	/// Register an editor session with its outbound event sink.
	///
	/// # Arguments
	/// * `session_id` - Unique identifier for the editor session.
	/// * `sink` - Communication channel back to the editor.
	pub fn register_session(&self, session_id: SessionId, sink: super::SessionSink) {
		let mut state = self.state.lock().unwrap();
		state.sessions.insert(
			session_id,
			super::SessionEntry {
				sink,
				attached: std::collections::HashSet::new(),
			},
		);
	}

	/// Unregister an editor session and detach from all servers.
	///
	/// Performs authoritative cleanup of the session state. It detaches
	/// the session from all running servers, potentially triggering idle leases,
	/// and cancels any pending requests where this session was the responder.
	pub fn unregister_session(self: &Arc<Self>, session_id: SessionId) {
		let mut servers_to_detach = Vec::new();
		{
			let mut state = self.state.lock().unwrap();
			if let Some(session) = state.sessions.remove(&session_id) {
				servers_to_detach.extend(session.attached);
			}
		}

		for server_id in servers_to_detach {
			self.detach_session(server_id, session_id);
			self.cleanup_session_docs_on_server(server_id, session_id);
		}

		self.cleanup_session_sync_docs(session_id);
		self.cancel_all_client_requests_for_session(session_id);
		self.cancel_all_c2s_requests_for_session(session_id);
	}

	pub(super) fn cleanup_session_docs_on_server(
		&self,
		server_id: ServerId,
		session_id: SessionId,
	) {
		let mut state = self.state.lock().unwrap();
		let Some(server) = state.servers.get_mut(&server_id) else {
			return;
		};

		let mut to_remove = Vec::new();
		for (uri, owner_state) in server.doc_owners.by_uri.iter_mut() {
			owner_state.open_refcounts.remove(&session_id);

			if owner_state.owner == session_id {
				// Re-elect owner or remove doc
				if let Some(&new_owner) = owner_state.open_refcounts.keys().min() {
					owner_state.owner = new_owner;
				} else {
					to_remove.push(uri.clone());
				}
			}
		}

		for uri in to_remove {
			server.doc_owners.by_uri.remove(&uri);
			server.docs.by_uri.remove(&uri);
		}
	}

	pub(super) fn cancel_all_c2s_requests_for_session(&self, session_id: SessionId) {
		let mut state = self.state.lock().unwrap();
		state
			.pending_c2s
			.retain(|_, req| req.origin_session != session_id);
	}

	/// Authoritatively cleans up a session that is determined to be dead.
	pub fn handle_session_send_failure(self: &Arc<Self>, session_id: SessionId) {
		tracing::warn!(?session_id, "session send failed, triggering cleanup");
		self.cancel_all_client_requests_for_session(session_id);
		self.unregister_session(session_id);
	}
}
