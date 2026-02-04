//! Focus tracking and activity reporting.

use std::time::Instant;

use xeno_broker_proto::types::{RequestPayload, SyncNonce};

use super::manager::SharedStateManager;
use super::types::{ACTIVITY_THROTTLE, SharedDocEntry, SharedStateRole};
use crate::buffer::DocumentId;

impl SharedStateManager {
	/// Generates a fresh nonce for correlation.
	pub(super) fn fresh_nonce(entry: &mut SharedDocEntry) -> SyncNonce {
		entry.next_nonce = entry.next_nonce.wrapping_add(1).max(1);
		SyncNonce(entry.next_nonce)
	}

	/// Records user activity for a document, returning a broker request if due.
	pub fn note_activity(&mut self, doc_id: DocumentId) -> Option<RequestPayload> {
		let uri = self.doc_id_to_uri.get(&doc_id)?.to_string();
		let entry = self.docs.get_mut(&uri)?;
		let now = Instant::now();

		if entry
			.last_activity_sent
			.is_some_and(|last| now.duration_since(last) < ACTIVITY_THROTTLE)
		{
			return None;
		}

		entry.last_activity_sent = Some(now);
		Some(RequestPayload::SharedActivity { uri })
	}

	/// Prepares a focus update request for a document.
	pub fn prepare_focus(
		&mut self,
		doc_id: DocumentId,
		focused: bool,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
	) -> Option<RequestPayload> {
		let uri = self.doc_id_to_uri.get(&doc_id)?.to_string();
		let entry = self.docs.get_mut(&uri)?;
		entry.focus_seq = entry.focus_seq.wrapping_add(1);

		let nonce = Self::fresh_nonce(entry);
		entry.pending_align = Some(nonce);

		Some(RequestPayload::SharedFocus {
			uri,
			focused,
			focus_seq: entry.focus_seq,
			nonce,
			client_hash64,
			client_len_chars,
		})
	}

	/// Returns the appropriate fingerprint to use for a focus claim.
	///
	/// If the local session is the current owner and is aligned with the broker,
	/// returns the authoritative cached fingerprint to prevent redundant repairs.
	/// Otherwise returns `(None, None)` to signal that the actual local rope
	/// fingerprint should be computed.
	pub fn focus_fingerprint_for_uri(&self, uri: &str) -> (Option<u64>, Option<u64>) {
		if let Some(entry) = self.docs.get(uri)
			&& entry.role == SharedStateRole::Owner
			&& !entry.needs_resync
		{
			return (Some(entry.auth_len_chars), Some(entry.auth_hash64));
		}
		(None, None)
	}
}
