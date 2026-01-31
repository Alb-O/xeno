//! Cross-process buffer synchronization manager.
//!
//! Tracks per-open-document sync state (epoch, seq, role) and provides methods
//! to prepare outgoing broker requests and process incoming broker events.

pub mod convert;

use std::collections::HashMap;

use xeno_broker_proto::types::{
	BufferSyncRole, RequestPayload, SessionId, SyncEpoch, SyncSeq, WireTx,
};
use xeno_primitives::Transaction;

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
	/// A delta request was rejected by the broker (seq/epoch mismatch or error).
	DeltaRejected {
		/// Document URI.
		uri: String,
	},
	/// Broker transport disconnected — disable all sync tracking.
	Disconnected,
}

/// Per-document sync state tracked by the editor.
struct SyncDocEntry {
	doc_id: DocumentId,
	epoch: SyncEpoch,
	seq: SyncSeq,
	role: BufferSyncRole,
	/// Set when an epoch mismatch or sequence gap is detected, indicating
	/// the local document may have diverged from the broker's authoritative
	/// copy. Cleared when a resync request is drained and sent.
	needs_resync: bool,
}

/// Manages buffer sync state for all documents in this editor session.
///
/// Converts between editor transactions and wire format, tracks ownership and
/// sequencing, and provides helpers for the editor to decide whether local edits
/// should be forwarded to the broker.
pub struct BufferSyncManager {
	/// URI → sync state.
	docs: HashMap<String, SyncDocEntry>,
	/// DocumentId → URI reverse lookup.
	doc_id_to_uri: HashMap<DocumentId, String>,
}

impl Default for BufferSyncManager {
	fn default() -> Self {
		Self::new()
	}
}

impl BufferSyncManager {
	/// Creates a new empty manager.
	pub fn new() -> Self {
		Self {
			docs: HashMap::new(),
			doc_id_to_uri: HashMap::new(),
		}
	}

	/// Prepares a [`RequestPayload::BufferSyncOpen`] and pre-registers the
	/// doc-id → URI reverse mapping for later lookup on response.
	pub fn prepare_open(&mut self, uri: &str, text: &str, doc_id: DocumentId) -> RequestPayload {
		self.doc_id_to_uri.insert(doc_id, uri.to_string());
		RequestPayload::BufferSyncOpen {
			uri: uri.to_string(),
			text: text.to_string(),
			version_hint: None,
		}
	}

	/// Processes the broker's `BufferSyncOpened` response.
	///
	/// Records the sync state. If the role is `Follower` and a snapshot is
	/// provided, returns the snapshot text so the caller can replace the
	/// local document content.
	pub fn handle_opened(
		&mut self,
		uri: &str,
		role: BufferSyncRole,
		epoch: SyncEpoch,
		seq: SyncSeq,
		snapshot: Option<String>,
	) -> Option<String> {
		let doc_id = self
			.doc_id_to_uri
			.iter()
			.find(|(_id, u)| u.as_str() == uri)
			.map(|(id, _u)| *id)?;

		self.docs.insert(
			uri.to_string(),
			SyncDocEntry {
				doc_id,
				epoch,
				seq,
				role,
				needs_resync: false,
			},
		);

		if role == BufferSyncRole::Follower {
			snapshot
		} else {
			None
		}
	}

	/// Prepares a `BufferSyncDelta` request if this session owns the document.
	///
	/// Serializes the transaction to wire format and returns the request payload.
	/// The local sequence is optimistically incremented so that rapid edits
	/// produce consecutive `base_seq` values without waiting for broker acks.
	/// Returns `None` if the document is not tracked or the session is a follower.
	pub fn prepare_delta(&mut self, uri: &str, tx: &Transaction) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if entry.role != BufferSyncRole::Owner || entry.needs_resync {
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

	/// Handles a `DeltaAck` from the broker.
	///
	/// Only advances the local sequence forward. The local seq may already be
	/// ahead due to optimistic incrementing in [`prepare_delta`], so stale acks
	/// that would regress the counter are ignored.
	pub fn handle_delta_ack(&mut self, uri: &str, seq: SyncSeq) {
		if let Some(entry) = self.docs.get_mut(uri)
			&& seq.0 > entry.seq.0
		{
			entry.seq = seq;
		}
	}

	/// Marks a document as needing resync after a delta rejection.
	///
	/// Suppresses further [`prepare_delta`] calls until a resync snapshot
	/// clears the flag, preventing repeated submissions against stale state.
	pub fn mark_needs_resync(&mut self, uri: &str) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.needs_resync = true;
		}
	}

	/// Validates an incoming remote delta and updates the local sequence.
	///
	/// Returns the [`DocumentId`] when epoch matches, sequence is contiguous
	/// (`entry.seq + 1`), and the local role is follower. The caller converts
	/// the wire transaction via [`convert::wire_to_tx`] using the actual
	/// document rope and applies it.
	///
	/// On epoch mismatch or sequence gap, sets [`SyncDocEntry::needs_resync`]
	/// (drained by [`drain_resync_requests`]) and returns `None`.
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

		if epoch != entry.epoch {
			tracing::warn!(
				uri,
				local_epoch = entry.epoch.0,
				remote_epoch = epoch.0,
				"Buffer sync epoch mismatch, requesting resync"
			);
			entry.needs_resync = true;
			return None;
		}

		let expected_seq = SyncSeq(entry.seq.0.wrapping_add(1));
		if seq != expected_seq {
			tracing::warn!(
				uri,
				expected = expected_seq.0,
				received = seq.0,
				"Buffer sync sequence gap, requesting resync"
			);
			entry.needs_resync = true;
			return None;
		}

		entry.seq = seq;
		Some(entry.doc_id)
	}

	/// Processes an ownership change event from the broker.
	///
	/// Resets the sequence to zero (new epoch starts fresh) and clears
	/// any pending resync flag since the new epoch supersedes it.
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
			entry.role = if owner == local_session {
				BufferSyncRole::Owner
			} else {
				BufferSyncRole::Follower
			};
			entry.needs_resync = false;
		}
	}

	/// Prepares a `BufferSyncClose` request and removes the document from tracking.
	///
	/// Returns `None` if the document is not tracked.
	pub fn prepare_close(&mut self, uri: &str) -> Option<RequestPayload> {
		let entry = self.docs.remove(uri)?;
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

	/// Collects URIs of documents that need a full resync from the broker.
	///
	/// Returns `BufferSyncResync` request payloads for each desynced document
	/// and clears their `needs_resync` flags. Called once per tick after
	/// draining inbound events.
	pub fn drain_resync_requests(&mut self) -> Vec<RequestPayload> {
		let mut requests = Vec::new();
		for (uri, entry) in &mut self.docs {
			if entry.needs_resync {
				entry.needs_resync = false;
				requests.push(RequestPayload::BufferSyncResync { uri: uri.clone() });
			}
		}
		requests
	}

	/// Disables all sync tracking, clearing all entries so the editor can
	/// resume local-only editing.
	pub fn disable_all(&mut self) {
		self.docs.clear();
		self.doc_id_to_uri.clear();
	}
}
