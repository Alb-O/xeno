//! Cross-process buffer synchronization manager.
//!
//! Tracks per-open-document sync state (epoch, seq, role) and provides methods
//! to prepare outgoing broker requests and process incoming broker events.

pub mod convert;

use std::collections::{HashMap, VecDeque};

use xeno_broker_proto::types::{
	BufferSyncOwnerConfirmStatus, BufferSyncOwnershipStatus, BufferSyncRole, RequestPayload,
	SessionId, SyncEpoch, SyncSeq, WireTx,
};
use xeno_primitives::{EditOrigin, Selection, Transaction, UndoPolicy};

use crate::buffer::DocumentId;

/// Inbound events from the broker transport for buffer sync.
#[derive(Debug)]
pub enum BufferSyncEvent {
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
		/// Document URI.
		uri: String,
		/// New ownership epoch.
		epoch: SyncEpoch,
		/// New owner session.
		owner: SessionId,
		/// Authoritative hash.
		hash64: u64,
		/// Authoritative length.
		len_chars: u64,
	},
	/// Result of an ownership request.
	OwnershipResult {
		/// Document URI.
		uri: String,
		/// Status of the request.
		status: BufferSyncOwnershipStatus,
		/// Current epoch.
		epoch: SyncEpoch,
		/// Current owner.
		owner: SessionId,
	},
	/// Result of an ownership confirmation.
	OwnerConfirmResult {
		/// Document URI.
		uri: String,
		/// Status of the confirmation.
		status: BufferSyncOwnerConfirmStatus,
		/// Current epoch.
		epoch: SyncEpoch,
		/// Current sequence.
		seq: SyncSeq,
		/// Current owner.
		owner: SessionId,
		/// Full text snapshot if mismatch.
		snapshot: Option<String>,
	},
	/// Broker responded to a BufferSyncOpen request.
	Opened {
		/// Document URI.
		uri: String,
		/// Assigned role.
		role: BufferSyncRole,
		/// Current epoch.
		epoch: SyncEpoch,
		/// Current sequence.
		seq: SyncSeq,
		/// Snapshot text if joining as follower.
		snapshot: Option<String>,
	},
	/// Broker acknowledged a delta.
	DeltaAck {
		/// Document URI.
		uri: String,
		/// New sequence number.
		seq: SyncSeq,
	},
	/// Full resync snapshot from broker.
	Snapshot {
		/// Document URI.
		uri: String,
		/// Full text content.
		text: String,
		/// Current epoch.
		epoch: SyncEpoch,
		/// Current sequence.
		seq: SyncSeq,
		/// Current owner session.
		owner: SessionId,
	},
	/// A delta request was rejected by the broker.
	DeltaRejected {
		/// Document URI.
		uri: String,
	},
	/// A request failed with a protocol error.
	RequestFailed {
		/// Document URI.
		uri: String,
	},
	/// Broker transport disconnected — disable all sync tracking.
	Disconnected,
}

/// An edit deferred because the session is not the document owner.
pub struct PendingEdit {
	/// Transaction to apply.
	pub tx: Transaction,
	/// Selection to apply after the transaction.
	pub selection: Option<Selection>,
	/// Undo policy for the edit.
	pub undo: UndoPolicy,
	/// Origin of the edit.
	pub origin: EditOrigin,
}

/// Outcome of attempting to apply an edit to a synced document.
pub enum DeferEditOutcome {
	/// Edit allowed to proceed immediately (session is owner and unblocked).
	Allowed,
	/// Edit deferred; an ownership request was prepared and should be sent.
	NeedTakeOwnership(RequestPayload),
	/// Edit deferred; an ownership acquisition or confirmation is already in flight.
	AlreadyAcquiring,
	/// Document is not currently tracked by the buffer sync manager.
	NotTracked,
}

/// Need for an ownership confirmation.
///
/// Generated when the broker grants ownership but the local session must prove
/// it is aligned with the authoritative document state before submitting deltas.
pub struct OwnerConfirmNeed {
	/// Canonical document URI.
	pub uri: String,
	/// Expected ownership generation.
	pub epoch: SyncEpoch,
	/// Local document identifier.
	pub doc_id: DocumentId,
}

/// An edit ready to be replayed after gaining ownership.
pub struct ReplayEdit {
	/// Local document identifier.
	pub doc_id: DocumentId,
	/// Transaction to apply.
	pub tx: Transaction,
	/// Final selection after the transaction.
	pub selection: Option<Selection>,
	/// Undo recording policy.
	pub undo: UndoPolicy,
	/// Metadata about the source of the edit.
	pub origin: EditOrigin,
}

/// Per-document sync state tracked by the editor.
struct SyncDocEntry {
	doc_id: DocumentId,
	epoch: SyncEpoch,
	seq: SyncSeq,
	role: BufferSyncRole,
	owner: SessionId,
	needs_resync: bool,
	resync_requested: bool,
	acquire_in_flight: bool,
	owner_confirm_required: bool,
	owner_confirm_in_flight: bool,
	pending_edits: VecDeque<PendingEdit>,
}

impl SyncDocEntry {
	fn is_blocked(&self) -> bool {
		self.role == BufferSyncRole::Follower
			|| self.needs_resync
			|| self.acquire_in_flight
			|| self.owner_confirm_required
			|| self.owner_confirm_in_flight
	}
}

/// Manages cross-process buffer synchronization for all open documents.
///
/// This manager maintains the state machine for transitioning between `Follower`
/// (read-only) and `Owner` (read-write) roles. It handles edit deferral,
/// optimistic sequencing, and convergence via alignment fingerprinting.
pub struct BufferSyncManager {
	docs: HashMap<String, SyncDocEntry>,
	uri_to_doc_id: HashMap<String, DocumentId>,
	doc_id_to_uri: HashMap<DocumentId, String>,
}

impl BufferSyncManager {
	/// Creates a new empty sync manager.
	pub fn new() -> Self {
		Self {
			docs: HashMap::new(),
			uri_to_doc_id: HashMap::new(),
			doc_id_to_uri: HashMap::new(),
		}
	}

	/// Returns true if mutations for the URI are currently prohibited.
	///
	/// Reasons for blocking include being a follower, awaiting ownership grant,
	/// or performing alignment confirmation.
	pub fn is_edit_blocked(&self, uri: &str) -> bool {
		self.docs.get(uri).is_some_and(SyncDocEntry::is_blocked)
	}

	/// Attempts to defer an edit for a blocked document.
	///
	/// If the document is not blocked, returns [`DeferEditOutcome::Allowed`].
	/// If it is a follower, prepares a `TakeOwnership` request.
	pub fn defer_edit(&mut self, uri: &str, edit: PendingEdit) -> DeferEditOutcome {
		let Some(entry) = self.docs.get_mut(uri) else {
			return DeferEditOutcome::NotTracked;
		};

		if !entry.is_blocked() {
			return DeferEditOutcome::Allowed;
		}

		entry.pending_edits.push_back(edit);

		if entry.role == BufferSyncRole::Follower && !entry.acquire_in_flight {
			entry.acquire_in_flight = true;
			DeferEditOutcome::NeedTakeOwnership(RequestPayload::BufferSyncTakeOwnership {
				uri: uri.to_string(),
			})
		} else {
			DeferEditOutcome::AlreadyAcquiring
		}
	}

	/// Collects and clears ownership confirmation requests.
	///
	/// Returns a list of documents where local ownership was granted but not
	/// yet confirmed via fingerprinting. Marks them as confirmation in-flight.
	pub fn drain_owner_confirm_requests(&mut self) -> Vec<OwnerConfirmNeed> {
		let mut needs = Vec::new();
		for (uri, entry) in &mut self.docs {
			if entry.owner_confirm_required && !entry.owner_confirm_in_flight {
				entry.owner_confirm_in_flight = true;
				needs.push(OwnerConfirmNeed {
					uri: uri.clone(),
					epoch: entry.epoch,
					doc_id: entry.doc_id,
				});
			}
		}
		needs
	}

	/// Collects and clears edits ready for replay.
	///
	/// Edits are returned for documents that are no longer blocked and have
	/// pending transactions in their queue.
	pub fn drain_replay_edits(&mut self) -> Vec<ReplayEdit> {
		let mut ready = Vec::new();
		for entry in self.docs.values_mut() {
			if !entry.is_blocked() {
				while let Some(pending) = entry.pending_edits.pop_front() {
					ready.push(ReplayEdit {
						doc_id: entry.doc_id,
						tx: pending.tx,
						selection: pending.selection,
						undo: pending.undo,
						origin: pending.origin,
					});
				}
			}
		}
		ready
	}

	/// Prepares a [`RequestPayload::BufferSyncOpen`] and registers document mappings.
	pub fn prepare_open(&mut self, uri: &str, text: &str, doc_id: DocumentId) -> RequestPayload {
		self.uri_to_doc_id.insert(uri.to_string(), doc_id);
		self.doc_id_to_uri.insert(doc_id, uri.to_string());
		RequestPayload::BufferSyncOpen {
			uri: uri.to_string(),
			text: text.to_string(),
			version_hint: None,
		}
	}

	/// Processes the broker's `BufferSyncOpened` response and initializes sync state.
	pub fn handle_opened(
		&mut self,
		uri: &str,
		role: BufferSyncRole,
		epoch: SyncEpoch,
		seq: SyncSeq,
		snapshot: Option<String>,
	) -> Option<String> {
		let doc_id = self.uri_to_doc_id.get(uri).copied()?;

		self.docs.insert(
			uri.to_string(),
			SyncDocEntry {
				doc_id,
				epoch,
				seq,
				role,
				owner: SessionId(0),
				needs_resync: false,
				resync_requested: false,
				acquire_in_flight: false,
				owner_confirm_required: false,
				owner_confirm_in_flight: false,
				pending_edits: VecDeque::new(),
			},
		);

		(role == BufferSyncRole::Follower)
			.then_some(snapshot)
			.flatten()
	}

	/// Prepares a [`RequestPayload::BufferSyncDelta`] if the session owns the document.
	///
	/// Optimistically increments the local sequence number. Returns `None` if
	/// the document is blocked or not owned.
	pub fn prepare_delta(&mut self, uri: &str, tx: &Transaction) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if entry.role != BufferSyncRole::Owner || entry.is_blocked() {
			return None;
		}

		let wire_tx = convert::tx_to_wire(tx);
		let base_seq = entry.seq;
		entry.seq = SyncSeq(entry.seq.0.wrapping_add(1));
		Some(RequestPayload::BufferSyncDelta {
			uri: uri.to_string(),
			epoch: entry.epoch,
			base_seq,
			tx: wire_tx,
		})
	}

	/// Handles a `DeltaAck` from the broker, advancing the local sequence.
	pub fn handle_delta_ack(&mut self, uri: &str, seq: SyncSeq) {
		if let Some(entry) = self.docs.get_mut(uri)
			&& seq.0 > entry.seq.0
		{
			entry.seq = seq;
		}
	}

	/// Marks a document as needing resync after a delta rejection.
	pub fn mark_needs_resync(&mut self, uri: &str) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.needs_resync = true;
			entry.resync_requested = false;
		}
	}

	/// Validates an incoming remote delta and updates the local sequence.
	///
	/// Returns the [`DocumentId`] if the delta is contiguous and applies cleanly.
	/// If an acquisition is in flight, all pending edits are invalidated and
	/// a resync is forced to ensure convergence.
	pub fn handle_remote_delta(
		&mut self,
		uri: &str,
		epoch: SyncEpoch,
		seq: SyncSeq,
	) -> Option<DocumentId> {
		let entry = self.docs.get_mut(uri)?;

		if entry.role == BufferSyncRole::Owner {
			return None;
		}

		if entry.is_blocked() || !entry.pending_edits.is_empty() {
			tracing::info!(
				uri,
				"Remote delta received during acquisition, invalidating pending edits"
			);
			entry.pending_edits.clear();
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}

		if epoch != entry.epoch {
			tracing::warn!(
				uri,
				local = entry.epoch.0,
				remote = epoch.0,
				"Epoch mismatch"
			);
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}

		let expected_seq = SyncSeq(entry.seq.0.wrapping_add(1));
		if seq != expected_seq {
			tracing::warn!(
				uri,
				expected = expected_seq.0,
				received = seq.0,
				"Sequence gap"
			);
			entry.needs_resync = true;
			entry.resync_requested = false;
			return None;
		}

		entry.seq = seq;
		Some(entry.doc_id)
	}

	/// Processes an ownership change event from the broker.
	///
	/// Resets the sequence number for the new epoch and clears any pending edits
	/// if the local session is not the new owner. If local session is the new
	/// owner, marks confirmation as required.
	pub fn handle_owner_changed(
		&mut self,
		uri: &str,
		epoch: SyncEpoch,
		owner: SessionId,
		local_session: SessionId,
	) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.epoch = epoch;
			entry.seq = SyncSeq(0);
			entry.owner = owner;
			entry.acquire_in_flight = false;

			if owner == local_session {
				entry.role = BufferSyncRole::Owner;
				entry.owner_confirm_required = true;
				entry.owner_confirm_in_flight = false;
				entry.needs_resync = false;
			} else {
				entry.role = BufferSyncRole::Follower;
				entry.owner_confirm_required = false;
				entry.owner_confirm_in_flight = false;
				entry.needs_resync = false;
				entry.resync_requested = false;
				entry.pending_edits.clear();
			}
		}
	}

	/// Handles an ownership request result.
	///
	/// Clears the acquisition flag and updates ownership state. If denied,
	/// all pending edits for the document are discarded.
	pub fn handle_ownership_result(
		&mut self,
		uri: &str,
		status: BufferSyncOwnershipStatus,
		epoch: SyncEpoch,
		owner: SessionId,
		local_session: SessionId,
	) {
		self.handle_owner_changed(uri, epoch, owner, local_session);

		if let Some(entry) = self.docs.get_mut(uri) {
			entry.acquire_in_flight = false;
			if status == BufferSyncOwnershipStatus::Denied {
				entry.pending_edits.clear();
			}
		}
	}

	/// Handles an ownership confirmation result.
	///
	/// If confirmed, clears the confirmation gate allowing replayed edits to
	/// proceed. If a snapshot is needed, clears the gate to allow the snapshot
	/// install to finalize the transition.
	pub fn handle_owner_confirm_result(
		&mut self,
		uri: &str,
		status: BufferSyncOwnerConfirmStatus,
		epoch: SyncEpoch,
		seq: SyncSeq,
		owner: SessionId,
		local_session: SessionId,
	) {
		if let Some(entry) = self.docs.get_mut(uri) {
			if entry.epoch != epoch {
				return;
			}

			entry.owner_confirm_in_flight = false;
			entry.owner = owner;

			if owner == local_session {
				entry.role = BufferSyncRole::Owner;
				match status {
					BufferSyncOwnerConfirmStatus::Confirmed => {
						entry.owner_confirm_required = false;
						entry.seq = seq;
					}
					BufferSyncOwnerConfirmStatus::NeedSnapshot => {
						entry.owner_confirm_required = false;
						entry.owner_confirm_in_flight = false;
						entry.pending_edits.clear();
					}
				}
			} else {
				entry.role = BufferSyncRole::Follower;
				entry.owner_confirm_required = false;
				entry.pending_edits.clear();
			}
		}
	}

	/// Replaces document content from a snapshot and resets sync state.
	pub fn handle_snapshot(
		&mut self,
		uri: &str,
		text: String,
		epoch: SyncEpoch,
		seq: SyncSeq,
		owner: SessionId,
		local_session: SessionId,
	) -> String {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.epoch = epoch;
			entry.seq = seq;
			entry.owner = owner;
			entry.role = if owner == local_session {
				BufferSyncRole::Owner
			} else {
				BufferSyncRole::Follower
			};
			entry.needs_resync = false;
			entry.resync_requested = false;
			entry.owner_confirm_required = false;
			entry.owner_confirm_in_flight = false;
			entry.acquire_in_flight = false;
			entry.pending_edits.clear();
		}
		text
	}

	/// Handles a failed request by clearing in-flight flags.
	pub fn handle_request_failed(&mut self, uri: &str) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.acquire_in_flight = false;
			entry.owner_confirm_in_flight = false;
			entry.resync_requested = false;
		}
	}

	/// Prepares a `BufferSyncClose` request and removes the document from tracking.
	pub fn prepare_close(&mut self, uri: &str) -> Option<RequestPayload> {
		let entry = self.docs.remove(uri)?;
		self.uri_to_doc_id.remove(uri);
		self.doc_id_to_uri.remove(&entry.doc_id);
		Some(RequestPayload::BufferSyncClose {
			uri: uri.to_string(),
		})
	}

	/// Returns `true` if the document is tracked as a follower.
	pub fn is_follower(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|e| e.role == BufferSyncRole::Follower)
	}

	/// Returns the URI associated with a document ID, if tracked.
	pub fn uri_for_doc_id(&self, doc_id: DocumentId) -> Option<&str> {
		self.doc_id_to_uri.get(&doc_id).map(String::as_str)
	}

	/// Returns the document ID associated with a URI, if tracked.
	pub fn doc_id_for_uri(&self, uri: &str) -> Option<DocumentId> {
		self.docs.get(uri).map(|e| e.doc_id)
	}

	/// Returns the role for a document URI, if tracked.
	pub fn role_for_uri(&self, uri: &str) -> Option<BufferSyncRole> {
		self.docs.get(uri).map(|e| e.role)
	}

	/// Returns the UI state for a document URI.
	pub fn ui_status_for_uri(&self, uri: &str) -> (Option<BufferSyncRole>, SyncStatus) {
		let Some(entry) = self.docs.get(uri) else {
			return (None, SyncStatus::Off);
		};

		let status = if entry.needs_resync {
			SyncStatus::NeedsResync
		} else if entry.acquire_in_flight {
			SyncStatus::Acquiring
		} else if entry.owner_confirm_required || entry.owner_confirm_in_flight {
			SyncStatus::Confirming
		} else if entry.role == BufferSyncRole::Owner {
			SyncStatus::Owner
		} else {
			SyncStatus::Follower
		};

		(Some(entry.role), status)
	}

	/// Collects URIs of documents that need a full resync from the broker.
	pub fn drain_resync_requests(&mut self) -> Vec<RequestPayload> {
		let mut requests = Vec::new();
		for (uri, entry) in &mut self.docs {
			if entry.needs_resync && !entry.resync_requested {
				entry.resync_requested = true;
				requests.push(RequestPayload::BufferSyncResync { uri: uri.clone() });
			}
		}
		requests
	}

	/// Clears the resync-required gate after a snapshot is applied.
	pub fn clear_needs_resync(&mut self, uri: &str) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.needs_resync = false;
			entry.resync_requested = false;
		}
	}

	/// Returns true if the document is currently blocked on resync.
	pub fn needs_resync(&self, uri: &str) -> bool {
		self.docs.get(uri).is_some_and(|entry| entry.needs_resync)
	}

	/// Disables all sync tracking.
	pub fn disable_all(&mut self) {
		self.docs.clear();
		self.uri_to_doc_id.clear();
		self.doc_id_to_uri.clear();
	}
}

/// UI-facing sync status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
	/// Buffer synchronization is disabled or disconnected.
	Off,
	/// Session is the authoritative owner/writer.
	Owner,
	/// Session is a read-only live viewer.
	Follower,
	/// Session is attempting to acquire ownership.
	Acquiring,
	/// Session is confirming alignment with the broker.
	Confirming,
	/// Session detected divergence and requires resync.
	NeedsResync,
}

impl Default for BufferSyncManager {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use xeno_primitives::Rope;
	use xeno_primitives::transaction::{Change, Transaction};

	use super::*;

	fn sample_tx() -> Transaction {
		let rope = Rope::from("hello");
		Transaction::change(
			rope.slice(..),
			std::iter::once(Change {
				start: 5,
				end: 5,
				replacement: Some("!".into()),
			}),
		)
	}

	#[test]
	fn test_owner_changed_sets_owner_confirm_required_for_local_new_owner() {
		let mut manager = BufferSyncManager::new();
		let uri = "file:///test.rs";
		let doc_id = DocumentId::next();

		manager.prepare_open(uri, "hello", doc_id);
		manager.handle_opened(
			uri,
			BufferSyncRole::Follower,
			SyncEpoch(1),
			SyncSeq(0),
			None,
		);

		manager.handle_owner_changed(uri, SyncEpoch(2), SessionId(1), SessionId(1));

		assert!(manager.is_edit_blocked(uri));
		assert!(manager.prepare_delta(uri, &sample_tx()).is_none());
		let needs = manager.drain_owner_confirm_requests();
		assert_eq!(needs.len(), 1);
		assert_eq!(needs[0].uri, uri);
	}

	#[test]
	fn test_confirm_clears_required_and_allows_delta() {
		let mut manager = BufferSyncManager::new();
		let uri = "file:///test.rs";
		let doc_id = DocumentId::next();

		manager.prepare_open(uri, "hello", doc_id);
		manager.handle_opened(
			uri,
			BufferSyncRole::Follower,
			SyncEpoch(1),
			SyncSeq(0),
			None,
		);

		manager.handle_owner_changed(uri, SyncEpoch(2), SessionId(1), SessionId(1));
		manager.drain_owner_confirm_requests();

		manager.handle_owner_confirm_result(
			uri,
			BufferSyncOwnerConfirmStatus::Confirmed,
			SyncEpoch(2),
			SyncSeq(0),
			SessionId(1),
			SessionId(1),
		);

		assert!(!manager.is_edit_blocked(uri));
		assert!(manager.prepare_delta(uri, &sample_tx()).is_some());
	}
}
