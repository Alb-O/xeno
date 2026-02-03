//! Shared document state manager for broker-backed synchronization.
//!
//! Tracks per-open-document sync state (epoch, seq, owner) and provides helpers
//! to prepare outgoing broker requests, emit activity/focus signals, and process
//! incoming broker events.

pub mod convert;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, RequestPayload, SessionId, SyncEpoch, SyncSeq, WireTx,
};
use xeno_primitives::Transaction;

use crate::buffer::DocumentId;

const ACTIVITY_THROTTLE: Duration = Duration::from_millis(750);

/// Request describing a resync for a shared document.
#[derive(Debug, Clone)]
pub struct ResyncRequest {
	/// Canonical document URI.
	pub uri: String,
	/// Local document identifier.
	pub doc_id: DocumentId,
}

/// Inbound events from the broker transport for shared state.
#[derive(Debug)]
pub enum SharedStateEvent {
	/// A remote delta was applied by the broker.
	RemoteDelta {
		/// Document URI.
		uri: String,
		/// Ownership epoch.
		epoch: SyncEpoch,
		/// New sequence number after this delta.
		seq: SyncSeq,
		/// The edit transaction in wire format.
		tx: WireTx,
	},
	/// Ownership of a document changed.
	OwnerChanged {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Preferred owner of a document changed.
	PreferredOwnerChanged {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Document ownership released (no current owner).
	Unlocked {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Broker responded to a SharedOpen request.
	Opened {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
		/// Snapshot text if joining as follower.
		text: Option<String>,
	},
	/// Broker acknowledged an edit.
	EditAck {
		/// Document URI.
		uri: String,
		/// Ownership epoch.
		epoch: SyncEpoch,
		/// New sequence number.
		seq: SyncSeq,
	},
	/// Full resync snapshot from broker.
	Snapshot {
		/// Document URI.
		uri: String,
		/// Full text content.
		text: String,
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Focus request acknowledged with updated snapshot.
	FocusAck {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// A request failed with a protocol error.
	RequestFailed {
		/// Document URI.
		uri: String,
	},
	/// A shared edit request was rejected by the broker.
	EditRejected {
		/// Document URI.
		uri: String,
	},
	/// Broker transport disconnected — disable all sync tracking.
	Disconnected,
}

/// Local role for a shared document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedStateRole {
	Owner,
	Follower,
}

/// UI status for a shared document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
	Off,
	Owner,
	Follower,
	Unlocked,
	NeedsResync,
}

#[derive(Debug, Clone, Copy)]
struct InFlightEdit {
	epoch: SyncEpoch,
	base_seq: SyncSeq,
}

/// Per-document sync state tracked by the editor.
struct SharedDocEntry {
	doc_id: DocumentId,
	epoch: SyncEpoch,
	seq: SyncSeq,
	role: SharedStateRole,
	owner: Option<SessionId>,
	preferred_owner: Option<SessionId>,
	phase: DocSyncPhase,
	needs_resync: bool,
	resync_requested: bool,
	open_refcount: u32,
	pending_deltas: VecDeque<WireTx>,
	in_flight: Option<InFlightEdit>,
	last_activity_sent: Option<std::time::Instant>,
	focus_seq: u64,
}

impl SharedDocEntry {
	fn is_blocked(&self) -> bool {
		self.role != SharedStateRole::Owner || self.needs_resync
	}
}

/// Manages broker-backed shared state for all open documents.
pub struct SharedStateManager {
	docs: HashMap<String, SharedDocEntry>,
	uri_to_doc_id: HashMap<String, DocumentId>,
	doc_id_to_uri: HashMap<DocumentId, String>,
}

impl SharedStateManager {
	/// Creates a new empty manager.
	pub fn new() -> Self {
		Self {
			docs: HashMap::new(),
			uri_to_doc_id: HashMap::new(),
			doc_id_to_uri: HashMap::new(),
		}
	}

	fn apply_snapshot_state(
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
		entry.role = if snapshot.owner == Some(local_session) {
			SharedStateRole::Owner
		} else {
			SharedStateRole::Follower
		};

		if entry.role == SharedStateRole::Owner {
			let diverged = snapshot.phase == DocSyncPhase::Diverged;
			entry.needs_resync = diverged;

			if diverged {
				// Re-enable drain_resync_requests and cancel local pipeline
				entry.resync_requested = false;
				entry.pending_deltas.clear();
				entry.in_flight = None;
			} else {
				entry.resync_requested = false;
			}
		} else if has_text {
			entry.needs_resync = false;
			entry.resync_requested = false;
		}

		if entry.role != SharedStateRole::Owner {
			entry.pending_deltas.clear();
			entry.in_flight = None;
		}
	}

	/// Returns true if mutations for the URI are currently prohibited.
	pub fn is_edit_blocked(&self, uri: &str) -> bool {
		self.docs.get(uri).is_some_and(SharedDocEntry::is_blocked)
	}

	/// Returns true if the local session is the current owner.
	pub fn is_owner(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|entry| entry.role == SharedStateRole::Owner)
	}

	/// Returns true if the document is currently unlocked.
	pub fn is_unlocked(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|entry| entry.phase == DocSyncPhase::Unlocked)
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

	/// Records focus for a document, returning a broker request if tracked.
	pub fn note_focus(&mut self, doc_id: DocumentId, focused: bool) -> Option<RequestPayload> {
		let uri = self.doc_id_to_uri.get(&doc_id)?.to_string();
		let entry = self.docs.get_mut(&uri)?;
		entry.focus_seq = entry.focus_seq.wrapping_add(1);
		Some(RequestPayload::SharedFocus {
			uri,
			focused,
			focus_seq: entry.focus_seq,
		})
	}

	/// Prepares a [`RequestPayload::SharedOpen`] and registers document mappings.
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
				in_flight: None,
				last_activity_sent: None,
				focus_seq: 0,
			});
		entry.doc_id = doc_id;
		entry.open_refcount = entry.open_refcount.saturating_add(1);

		RequestPayload::SharedOpen {
			uri: uri.to_string(),
			text: text.to_string(),
			version_hint: None,
		}
	}

	/// Prepares a `SharedClose` request and removes the document if needed.
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

	/// Processes the broker's `SharedOpened` response and initializes sync state.
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
				in_flight: None,
				last_activity_sent: None,
				focus_seq: 0,
			});
		entry.doc_id = doc_id;
		Self::apply_snapshot_state(entry, &snapshot, local_session, text.is_some());
		text
	}

	/// Prepares a [`RequestPayload::SharedEdit`] if the session owns the document.
	pub fn prepare_edit(&mut self, uri: &str, tx: &Transaction) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if entry.role != SharedStateRole::Owner || entry.needs_resync {
			return None;
		}

		let wire = convert::tx_to_wire(tx);
		if entry.in_flight.is_some() {
			entry.pending_deltas.push_back(wire);
			return None;
		}

		entry.in_flight = Some(InFlightEdit {
			epoch: entry.epoch,
			base_seq: entry.seq,
		});
		Some(RequestPayload::SharedEdit {
			uri: uri.to_string(),
			epoch: entry.epoch,
			base_seq: entry.seq,
			tx: wire,
		})
	}

	/// Handles an edit acknowledgment from the broker, advancing the local sequence.
	pub fn handle_edit_ack(&mut self, uri: &str, epoch: SyncEpoch, seq: SyncSeq) {
		if let Some(entry) = self.docs.get_mut(uri)
			&& let Some(in_flight) = entry.in_flight {
				let expected = in_flight.base_seq.0.wrapping_add(1);
				if epoch == in_flight.epoch && seq.0 == expected {
					entry.seq = seq;
					entry.in_flight = None;
				} else {
					tracing::warn!(
						?uri,
						ack_epoch = ?epoch,
						ack_seq = ?seq,
						in_flight_epoch = ?in_flight.epoch,
						in_flight_base = ?in_flight.base_seq,
						"stale or mismatched SharedEditAck ignored"
					);
				}
			}
	}

	/// Collects queued edit requests once the in-flight delta is acknowledged.
	pub fn drain_pending_edit_requests(&mut self) -> Vec<RequestPayload> {
		let mut requests = Vec::new();
		for (uri, entry) in &mut self.docs {
			if entry.in_flight.is_some()
				|| entry.pending_deltas.is_empty()
				|| entry.role != SharedStateRole::Owner
				|| entry.needs_resync
			{
				continue;
			}

			if let Some(tx) = entry.pending_deltas.pop_front() {
				entry.in_flight = Some(InFlightEdit {
					epoch: entry.epoch,
					base_seq: entry.seq,
				});
				requests.push(RequestPayload::SharedEdit {
					uri: uri.clone(),
					epoch: entry.epoch,
					base_seq: entry.seq,
					tx,
				});
			}
		}
		requests
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

	/// Validates an incoming remote delta and updates the local sequence.
	pub fn handle_remote_delta(
		&mut self,
		uri: &str,
		epoch: SyncEpoch,
		seq: SyncSeq,
	) -> Option<DocumentId> {
		let entry = self.docs.get_mut(uri)?;
		if entry.role == SharedStateRole::Owner || entry.needs_resync {
			return None;
		}

		if epoch != entry.epoch {
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}

		let expected_seq = SyncSeq(entry.seq.0.wrapping_add(1));
		if seq != expected_seq {
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}

		entry.seq = seq;
		Some(entry.doc_id)
	}

	/// Applies a broker snapshot update for ownership or preferred owner changes.
	pub fn handle_snapshot_update(&mut self, snapshot: DocStateSnapshot, local_session: SessionId) {
		if let Some(entry) = self.docs.get_mut(&snapshot.uri) {
			Self::apply_snapshot_state(entry, &snapshot, local_session, false);
		}
	}

	/// Applies a full resync snapshot from the broker.
	pub fn handle_snapshot(
		&mut self,
		uri: &str,
		snapshot: DocStateSnapshot,
		local_session: SessionId,
	) {
		if let Some(entry) = self.docs.get_mut(uri) {
			Self::apply_snapshot_state(entry, &snapshot, local_session, true);
		}
	}

	/// Collects and clears resync requests for diverged documents.
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

	/// Handles protocol errors for a document.
	pub fn handle_request_failed(&mut self, uri: &str) {
		let Some(entry) = self.docs.get_mut(uri) else {
			return;
		};
		if entry.needs_resync {
			entry.resync_requested = false;
		} else {
			entry.pending_deltas.clear();
			entry.in_flight = None;
		}
	}

	/// Returns the local role for a document URI.
	pub fn role_for_uri(&self, uri: &str) -> Option<SharedStateRole> {
		self.docs.get(uri).map(|entry| entry.role)
	}

	/// Returns the status for UI display.
	pub fn ui_status_for_uri(&self, uri: &str) -> (Option<SharedStateRole>, SyncStatus) {
		let Some(entry) = self.docs.get(uri) else {
			return (None, SyncStatus::Off);
		};

		let status = if entry.needs_resync {
			SyncStatus::NeedsResync
		} else if entry.phase == DocSyncPhase::Unlocked {
			SyncStatus::Unlocked
		} else if entry.role == SharedStateRole::Owner {
			SyncStatus::Owner
		} else {
			SyncStatus::Follower
		};
		(Some(entry.role), status)
	}

	/// Returns the URI for a document id.
	pub fn uri_for_doc_id(&self, doc_id: DocumentId) -> Option<&str> {
		self.doc_id_to_uri.get(&doc_id).map(String::as_str)
	}

	/// Returns the doc id for a document URI.
	pub fn doc_id_for_uri(&self, uri: &str) -> Option<DocumentId> {
		self.uri_to_doc_id.get(uri).copied()
	}

	/// Disables all shared state tracking (e.g., broker disconnect).
	pub fn disable_all(&mut self) {
		self.docs.clear();
		self.uri_to_doc_id.clear();
		self.doc_id_to_uri.clear();
	}
}

/// Decides whether a snapshot payload should replace local content.
pub(crate) fn should_apply_snapshot_text(
	text: &str,
	snapshot: &DocStateSnapshot,
	local_len: Option<u64>,
	local_hash: Option<u64>,
) -> bool {
	if !text.is_empty() {
		return true;
	}

	match (local_len, local_hash) {
		(Some(len), Some(hash)) => !(len == snapshot.len_chars && hash == snapshot.hash64),
		_ => true,
	}
}

impl Default for SharedStateManager {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_snapshot_apply_when_text_present() {
		let snapshot = DocStateSnapshot {
			uri: "file:///test.rs".to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(2),
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Unlocked,
			hash64: 42,
			len_chars: 5,
		};

		assert!(should_apply_snapshot_text(
			"content",
			&snapshot,
			Some(5),
			Some(42)
		));
	}

	#[test]
	fn test_snapshot_apply_skips_matching_empty() {
		let snapshot = DocStateSnapshot {
			uri: "file:///test.rs".to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(2),
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Owned,
			hash64: 99,
			len_chars: 10,
		};

		assert!(!should_apply_snapshot_text(
			"",
			&snapshot,
			Some(10),
			Some(99)
		));
	}

	#[test]
	fn test_snapshot_apply_on_empty_mismatch() {
		let snapshot = DocStateSnapshot {
			uri: "file:///test.rs".to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(2),
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Owned,
			hash64: 99,
			len_chars: 10,
		};

		assert!(should_apply_snapshot_text(
			"",
			&snapshot,
			Some(10),
			Some(100)
		));
	}

	#[test]
	fn test_snapshot_apply_on_empty_without_fingerprint() {
		let snapshot = DocStateSnapshot {
			uri: "file:///test.rs".to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(2),
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Owned,
			hash64: 99,
			len_chars: 10,
		};

		assert!(should_apply_snapshot_text("", &snapshot, None, None));
	}
}
