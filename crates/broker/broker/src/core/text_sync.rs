//! Text synchronization gating logic.
//!
//! Enforces single-writer ownership per document URI for LSP text sync notifications.

use std::collections::HashMap;

use xeno_broker_proto::types::{DocId, ServerId, SessionId};

use super::BrokerCore;

#[derive(Debug, Default)]
pub(super) struct DocRegistry {
	/// Map of URI to (DocId, last_version).
	pub(super) by_uri: HashMap<String, (DocId, u32)>,
	next_doc_id: u64,
}

impl DocRegistry {
	pub(super) fn update(&mut self, uri: String, version: u32) {
		if let Some(info) = self.by_uri.get_mut(&uri) {
			info.1 = version;
		} else {
			let id = DocId(self.next_doc_id);
			self.next_doc_id += 1;
			self.by_uri.insert(uri, (id, version));
		}
	}
}

#[derive(Debug, Default)]
pub(super) struct DocOwnerRegistry {
	pub(super) by_uri: HashMap<String, DocOwnerState>,
}

#[derive(Debug)]
pub(super) struct DocOwnerState {
	/// Session that owns this document.
	pub(super) owner: SessionId,
	/// Per-session open refcount.
	pub(super) open_refcounts: HashMap<SessionId, u32>,
	/// Last observed version from the owner.
	pub(super) last_version: u32,
}

/// Result of gating a text sync notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocGateDecision {
	/// Forward to server: session is owner or first open.
	Forward,
	/// Drop silently: session is follower.
	DropSilently,
	/// Reject: session is not permitted to sync.
	RejectNotOwner,
}

/// Result struct for document gating operations.
pub struct DocGateResult {
	/// Decision (allow/reject).
	pub decision: DocGateDecision,
	/// URI being gated.
	pub uri: String,
	/// Kind of operation.
	pub kind: DocGateKind,
}

/// Kind of text sync operation.
pub enum DocGateKind {
	/// Document opened.
	DidOpen {
		/// New version.
		version: u32,
	},
	/// Document changed.
	DidChange {
		/// New version.
		version: u32,
	},
	/// Document closed.
	DidClose,
}

impl BrokerCore {
	/// Retrieve the DocId and last known version for a URI.
	pub fn get_doc_by_uri(&self, server_id: ServerId, uri: &str) -> Option<(DocId, u32)> {
		let routing = self.routing.lock().unwrap();
		let server = routing.servers.get(&server_id)?;
		server.docs.by_uri.get(uri).cloned()
	}

	/// Gate a text synchronization notification to enforce single-writer ownership per URI.
	///
	/// This ensures only the owner session (first opener or elected successor) can send
	/// modifications to the server.
	pub fn gate_text_sync(
		&self,
		session_id: SessionId,
		server_id: ServerId,
		notif: &xeno_lsp::AnyNotification,
	) -> DocGateDecision {
		let mut routing = self.routing.lock().unwrap();
		let Some(server) = routing.servers.get_mut(&server_id) else {
			return DocGateDecision::RejectNotOwner;
		};

		match notif.method.as_str() {
			"textDocument/didOpen" => {
				let Some(doc) = notif.params.get("textDocument") else {
					return DocGateDecision::RejectNotOwner;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return DocGateDecision::RejectNotOwner;
				};
				let version = doc.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

				match server.doc_owners.by_uri.get_mut(uri) {
					None => {
						let mut refcounts = HashMap::new();
						refcounts.insert(session_id, 1);
						server.doc_owners.by_uri.insert(
							uri.to_string(),
							DocOwnerState {
								owner: session_id,
								open_refcounts: refcounts,
								last_version: version,
							},
						);
						DocGateDecision::Forward
					}
					Some(owner_state) => {
						let count = owner_state.open_refcounts.entry(session_id).or_insert(0);
						*count += 1;

						if !server.attached.contains(&owner_state.owner)
							|| !owner_state.open_refcounts.contains_key(&owner_state.owner)
						{
							owner_state.owner = session_id;
						}

						DocGateDecision::DropSilently
					}
				}
			}
			"textDocument/didChange" => {
				let Some(doc) = notif.params.get("textDocument") else {
					return DocGateDecision::RejectNotOwner;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return DocGateDecision::RejectNotOwner;
				};
				let version = doc.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

				match server.doc_owners.by_uri.get_mut(uri) {
					None => DocGateDecision::RejectNotOwner,
					Some(owner_state) => {
						if session_id == owner_state.owner {
							owner_state.last_version = version;
							DocGateDecision::Forward
						} else if !server.attached.contains(&owner_state.owner) {
							owner_state.owner = session_id;
							owner_state.last_version = version;
							DocGateDecision::Forward
						} else {
							DocGateDecision::RejectNotOwner
						}
					}
				}
			}
			"textDocument/didClose" => {
				let Some(doc) = notif.params.get("textDocument") else {
					return DocGateDecision::RejectNotOwner;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return DocGateDecision::RejectNotOwner;
				};

				match server.doc_owners.by_uri.get_mut(uri) {
					None => DocGateDecision::RejectNotOwner,
					Some(owner_state) => {
						if let Some(count) = owner_state.open_refcounts.get_mut(&session_id) {
							if *count > 0 {
								*count -= 1;
							}
							if *count == 0 {
								owner_state.open_refcounts.remove(&session_id);
							}
						}

						if session_id == owner_state.owner
							&& !owner_state.open_refcounts.is_empty()
							&& let Some(&new_owner) = owner_state.open_refcounts.keys().min()
						{
							owner_state.owner = new_owner;
						}

						let global_count: u32 = owner_state.open_refcounts.values().sum();
						if global_count == 0 {
							server.doc_owners.by_uri.remove(uri);
							server.docs.by_uri.remove(uri);
							DocGateDecision::Forward
						} else {
							DocGateDecision::DropSilently
						}
					}
				}
			}
			_ => DocGateDecision::Forward,
		}
	}
}
