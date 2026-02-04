//! Document open/close lifecycle management.

use std::collections::VecDeque;

use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, RequestPayload, SessionId, SyncEpoch, SyncSeq,
};

use super::manager::SharedStateManager;
use super::types::{SharedDocEntry, SharedStateRole, SharedViewHistory};
use crate::buffer::DocumentId;

impl SharedStateManager {
	/// Registers document mappings and prepares an initial open request.
	pub fn prepare_open(&mut self, uri: &str, text: &str, doc_id: DocumentId) -> RequestPayload {
		self.uri_to_doc_id.insert(uri.to_string(), doc_id);
		self.doc_id_to_uri.insert(doc_id, uri.to_string());

		let entry = self
			.docs
			.entry(uri.to_string())
			.or_insert_with(|| SharedDocEntry {
				doc_id,
				epoch: SyncEpoch(0),
				seq: SyncSeq(0),
				role: SharedStateRole::Follower,
				owner: None,
				preferred_owner: None,
				phase: DocSyncPhase::Unlocked,
				needs_resync: false,
				resync_requested: false,
				open_refcount: 0,
				pending_deltas: VecDeque::new(),
				pending_history: VecDeque::new(),
				in_flight: None,
				last_activity_sent: None,
				focus_seq: 0,
				next_nonce: 1,
				pending_align: None,
				auth_hash64: 0,
				auth_len_chars: 0,
				current_undo_group: 1,
				view_history: SharedViewHistory::default(),
			});
		entry.doc_id = doc_id;
		entry.open_refcount = entry.open_refcount.saturating_add(1);

		RequestPayload::SharedOpen {
			uri: uri.to_string(),
			text: text.to_string(),
			version_hint: None,
		}
	}

	/// Prepares a close request and removes internal tracking if the refcount reaches zero.
	pub fn prepare_close(&mut self, uri: &str) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if entry.open_refcount > 0 {
			entry.open_refcount -= 1;
		}
		if entry.open_refcount == 0 {
			self.docs.remove(uri);
			if let Some(doc_id) = self.uri_to_doc_id.remove(uri) {
				self.doc_id_to_uri.remove(&doc_id);
			}
		}
		Some(RequestPayload::SharedClose {
			uri: uri.to_string(),
		})
	}

	/// Processes a `SharedOpened` response and initializes local sync state.
	pub fn handle_opened(
		&mut self,
		snapshot: DocStateSnapshot,
		text: Option<String>,
		local_session: SessionId,
	) -> Option<String> {
		let doc_id = self.uri_to_doc_id.get(&snapshot.uri).copied()?;
		let entry = self
			.docs
			.entry(snapshot.uri.clone())
			.or_insert_with(|| SharedDocEntry {
				doc_id,
				epoch: snapshot.epoch,
				seq: snapshot.seq,
				role: SharedStateRole::Follower,
				owner: snapshot.owner,
				preferred_owner: snapshot.preferred_owner,
				phase: snapshot.phase,
				needs_resync: false,
				resync_requested: false,
				open_refcount: 1,
				pending_deltas: VecDeque::new(),
				pending_history: VecDeque::new(),
				in_flight: None,
				last_activity_sent: None,
				focus_seq: 0,
				next_nonce: 1,
				pending_align: None,
				auth_hash64: snapshot.hash64,
				auth_len_chars: snapshot.len_chars,
				current_undo_group: 1,
				view_history: SharedViewHistory::default(),
			});
		entry.doc_id = doc_id;
		Self::apply_snapshot_state(entry, &snapshot, local_session, text.is_some());
		text
	}
}
