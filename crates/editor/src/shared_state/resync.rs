//! Resync request handling and failure recovery.

use xeno_broker_proto::types::{RequestPayload, SyncEpoch};

use super::manager::SharedStateManager;
use super::types::ResyncRequest;

impl SharedStateManager {
	/// Prepares a resync request for a diverged document.
	pub fn prepare_resync(
		&mut self,
		uri: &str,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
	) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		let nonce = Self::fresh_nonce(entry);
		entry.pending_align = Some(nonce);

		Some(RequestPayload::SharedResync {
			uri: uri.to_string(),
			nonce,
			client_hash64,
			client_len_chars,
		})
	}

	/// Marks a document as needing resync after a delta rejection.
	pub fn mark_needs_resync(&mut self, uri: &str) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.needs_resync = true;
			entry.resync_requested = false;
			entry.pending_deltas.clear();
			entry.in_flight = None;
		}
	}

	/// Collects resync requests for diverged documents.
	pub fn drain_resync_requests(&mut self) -> Vec<ResyncRequest> {
		let mut requests = Vec::new();
		for (uri, entry) in &mut self.docs {
			if entry.needs_resync && !entry.resync_requested {
				entry.resync_requested = true;
				requests.push(ResyncRequest {
					uri: uri.clone(),
					doc_id: entry.doc_id,
				});
			}
		}
		requests
	}

	/// Handles protocol errors by resetting internal pipeline guards.
	pub fn handle_request_failed(&mut self, uri: &str) {
		let Some(entry) = self.docs.get_mut(uri) else {
			return;
		};

		if entry.epoch == SyncEpoch(0) {
			let doc_id = entry.doc_id;
			self.docs.remove(uri);
			self.uri_to_doc_id.remove(uri);
			self.doc_id_to_uri.remove(&doc_id);
			return;
		}

		if entry.needs_resync {
			entry.resync_requested = false;
		} else {
			entry.pending_deltas.clear();
			entry.in_flight = None;
		}
		entry.pending_align = None;
	}
}
