//! Snapshot and remote delta synchronization.

use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, SessionId, SyncEpoch, SyncNonce, SyncSeq,
};

use super::manager::SharedStateManager;
use super::types::{SharedDocEntry, SharedStateRole};
use crate::buffer::DocumentId;

impl SharedStateManager {
	/// Synchronizes an internal entry with the provided broker snapshot.
	///
	/// # Invariants
	///
	/// If authoritative text is installed locally (`has_text` is `true`), the edit pipeline
	/// is unconditionally cleared. This ensures that the owner does not attempt to
	/// publish deltas built against stale local content after an authoritative repair
	/// or snapshot.
	pub(super) fn apply_snapshot_state(
		entry: &mut SharedDocEntry,
		snapshot: &DocStateSnapshot,
		local_session: SessionId,
		has_text: bool,
	) {
		entry.epoch = snapshot.epoch;
		entry.seq = snapshot.seq;
		entry.owner = snapshot.owner;
		entry.preferred_owner = snapshot.preferred_owner;
		entry.phase = snapshot.phase;

		entry.auth_hash64 = snapshot.hash64;
		entry.auth_len_chars = snapshot.len_chars;

		entry.role = if snapshot.owner == Some(local_session) {
			SharedStateRole::Owner
		} else {
			SharedStateRole::Follower
		};

		if let Some(group) = snapshot.history_head_group {
			if group > 0 {
				entry.current_undo_group = group;
			}
		}

		if has_text {
			entry.pending_deltas.clear();
			entry.pending_history.clear();
			entry.in_flight = None;
			entry.needs_resync = false;
			entry.resync_requested = false;
		} else if entry.role == SharedStateRole::Owner {
			let diverged = snapshot.phase == DocSyncPhase::Diverged;
			entry.needs_resync = diverged;

			if diverged {
				entry.resync_requested = false;
				entry.pending_deltas.clear();
				entry.pending_history.clear();
				entry.in_flight = None;
			}
		}

		if entry.role != SharedStateRole::Owner {
			entry.pending_deltas.clear();
			entry.pending_history.clear();
			entry.in_flight = None;
		}
	}

	/// Returns true if the provided snapshot represents a state advanced from local tracking.
	pub(super) fn snapshot_is_newer(entry: &SharedDocEntry, snap: &DocStateSnapshot) -> bool {
		snap.epoch > entry.epoch || (snap.epoch == entry.epoch && snap.seq >= entry.seq)
	}

	/// Validates an incoming remote delta and updates the local sequence.
	pub fn handle_remote_delta(
		&mut self,
		uri: &str,
		epoch: SyncEpoch,
		seq: SyncSeq,
		hash64: u64,
		len_chars: u64,
	) -> Option<DocumentId> {
		let entry = self.docs.get_mut(uri)?;
		if entry.needs_resync {
			return None;
		}
		if entry.role == SharedStateRole::Owner {
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}
		if epoch != entry.epoch {
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}
		let expected = SyncSeq(entry.seq.0.wrapping_add(1));
		if seq != expected {
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}
		entry.seq = seq;
		entry.auth_hash64 = hash64;
		entry.auth_len_chars = len_chars;

		Some(entry.doc_id)
	}

	/// Applies an async snapshot update for document state changes.
	pub fn handle_snapshot_update(&mut self, snapshot: DocStateSnapshot, local_session: SessionId) {
		if let Some(entry) = self.docs.get_mut(&snapshot.uri) {
			Self::apply_snapshot_state(entry, &snapshot, local_session, false);
		}
	}

	/// Handles a `FocusAck` response.
	///
	/// Returns authoritative `repair_text` if a repair is required and correlated.
	pub fn handle_focus_ack(
		&mut self,
		snapshot: DocStateSnapshot,
		nonce: SyncNonce,
		repair_text: Option<String>,
		local_session: SessionId,
	) -> Option<String> {
		let entry = self.docs.get_mut(&snapshot.uri)?;
		let nonce_match = entry.pending_align == Some(nonce);
		let newer = Self::snapshot_is_newer(entry, &snapshot);

		if nonce_match || newer {
			if nonce_match {
				entry.pending_align = None;
			}
			Self::apply_snapshot_state(
				entry,
				&snapshot,
				local_session,
				repair_text.is_some() && nonce_match,
			);
			if nonce_match && let Some(text) = repair_text {
				let text_included = !text.is_empty() || snapshot.len_chars == 0;
				return text_included.then_some(text);
			}
		}
		None
	}

	/// Handles a `SharedSnapshot` response.
	///
	/// Returns the authoritative text if the response is correlated or advanced.
	pub fn handle_snapshot_response(
		&mut self,
		uri: &str,
		snapshot: DocStateSnapshot,
		nonce: SyncNonce,
		text: String,
		local_session: SessionId,
	) -> Option<String> {
		let entry = self.docs.get_mut(uri)?;
		let nonce_match = entry.pending_align == Some(nonce);
		let newer = Self::snapshot_is_newer(entry, &snapshot);

		if nonce_match || newer {
			if nonce_match {
				entry.pending_align = None;
			}
			Self::apply_snapshot_state(entry, &snapshot, local_session, true);
			let text_included = !text.is_empty() || snapshot.len_chars == 0;
			return text_included.then_some(text);
		}
		None
	}
}
