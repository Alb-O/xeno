//! Shared document state manager for broker-backed synchronization.
//!
//! The [`SharedStateManager`] tracks the synchronization lifecycle for every
//! shared document open in the editor. It manages ownership transitions,
//! edit sequencing, and ensures local state remains aligned with the broker's
//! authoritative truth using nonces and fingerprints.

pub mod convert;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, RequestPayload, SessionId, SharedApplyKind, SyncEpoch,
	SyncNonce, SyncSeq, WireTx,
};
use xeno_primitives::Transaction;

use crate::buffer::{DocumentId, ViewId};
use crate::types::ViewSnapshot;

/// Minimum interval between sending activity heartbeats to the broker.
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
		/// Kind of mutation.
		kind: SharedApplyKind,
		/// The edit transaction in wire format.
		tx: WireTx,
		/// Authority fingerprint after apply.
		hash64: u64,
		/// Authority length after apply.
		len_chars: u64,
		/// Previous history head node identifier.
		history_from_id: Option<u64>,
		/// New history head node identifier.
		history_to_id: Option<u64>,
		/// History group identifier affected by this operation.
		history_group: Option<u64>,
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
	/// Broker acknowledged an application (Edit/Undo/Redo).
	ApplyAck {
		/// Document URI.
		uri: String,
		/// Kind of mutation.
		kind: SharedApplyKind,
		/// Ownership epoch.
		epoch: SyncEpoch,
		/// New sequence number.
		seq: SyncSeq,
		/// Optional transaction to apply locally (Undo/Redo).
		applied_tx: Option<WireTx>,
		/// Authority fingerprint after apply.
		hash64: u64,
		/// Authority length after apply.
		len_chars: u64,
		/// Previous history head node identifier.
		history_from_id: Option<u64>,
		/// New history head node identifier.
		history_to_id: Option<u64>,
		/// History group identifier affected by this operation.
		history_group: Option<u64>,
	},
	/// Full resync snapshot from broker.
	Snapshot {
		/// Document URI.
		uri: String,
		/// Nonce echoed from request.
		nonce: SyncNonce,
		/// Full text content.
		text: String,
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Focus request acknowledged with updated snapshot.
	FocusAck {
		/// Nonce echoed from request.
		nonce: SyncNonce,
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
		/// Authoritative text if repair is needed.
		repair_text: Option<String>,
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
	/// Broker reported no undo history available.
	NothingToUndo {
		/// Document URI.
		uri: String,
	},
	/// Broker reported no redo history available.
	NothingToRedo {
		/// Document URI.
		uri: String,
	},
	/// Broker reported history is unavailable (e.g. storage disabled or corrupted).
	HistoryUnavailable {
		/// Document URI.
		uri: String,
	},
	/// Broker transport disconnected — disable all sync tracking.
	Disconnected,
}

/// Local role for a shared document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedStateRole {
	/// Local session has write authority.
	Owner,
	/// Local session follows authoritative changes.
	Follower,
}

/// UI status for a shared document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
	/// Document is not tracked by the broker.
	Off,
	/// Local session owns the document.
	Owner,
	/// Local session is following the document.
	Follower,
	/// Document is open but has no current owner.
	Unlocked,
	/// Document state has diverged and requires a resync.
	NeedsResync,
}

#[derive(Debug, Clone, Copy)]
struct InFlightEdit {
	epoch: SyncEpoch,
	base_seq: SyncSeq,
}

/// Per-group view state for exact cursor restoration.
#[derive(Debug, Clone, Default)]
pub struct GroupViewState {
	/// Snapshots before the group's first delta.
	pub pre: HashMap<ViewId, ViewSnapshot>,
	/// Snapshots after the group completes.
	pub post: HashMap<ViewId, ViewSnapshot>,
}

/// Local cache of view state indexed by broker undo group.
#[derive(Debug, Clone, Default)]
pub struct SharedViewHistory {
	/// Keyed by group_id.
	pub groups: HashMap<u64, GroupViewState>,
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
	pending_deltas: VecDeque<(WireTx, u64)>,
	in_flight: Option<InFlightEdit>,
	last_activity_sent: Option<Instant>,
	focus_seq: u64,
	next_nonce: u64,
	pending_align: Option<SyncNonce>,

	/// Authoritative fingerprint for current (epoch, seq).
	auth_hash64: u64,
	/// Authoritative length for current (epoch, seq).
	auth_len_chars: u64,

	/// Current local undo group identifier.
	current_undo_group: u64,
	/// View history cache for group-level undo/redo.
	view_history: SharedViewHistory,
}

impl SharedDocEntry {
	/// Returns true if mutations are currently prohibited due to role or divergence.
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

	/// Synchronizes an internal entry with the provided broker snapshot.
	///
	/// # Invariants
	///
	/// If authoritative text is installed locally (`has_text` is `true`), the edit pipeline
	/// is unconditionally cleared. This ensures that the owner does not attempt to
	/// publish deltas built against stale local content after an authoritative repair
	/// or snapshot.
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

		entry.auth_hash64 = snapshot.hash64;
		entry.auth_len_chars = snapshot.len_chars;

		entry.role = if snapshot.owner == Some(local_session) {
			SharedStateRole::Owner
		} else {
			SharedStateRole::Follower
		};

		if has_text {
			entry.pending_deltas.clear();
			entry.in_flight = None;
			entry.needs_resync = false;
			entry.resync_requested = false;
		} else if entry.role == SharedStateRole::Owner {
			let diverged = snapshot.phase == DocSyncPhase::Diverged;
			entry.needs_resync = diverged;

			if diverged {
				entry.resync_requested = false;
				entry.pending_deltas.clear();
				entry.in_flight = None;
			}
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

	fn fresh_nonce(entry: &mut SharedDocEntry) -> SyncNonce {
		entry.next_nonce = entry.next_nonce.wrapping_add(1).max(1);
		SyncNonce(entry.next_nonce)
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

	/// Prepares an authoritative mutation request.
	///
	/// Pipelining: If a request is already in-flight, edits are queued in
	/// `pending_deltas` to be drained after acknowledgment.
	fn prepare_apply(
		&mut self,
		uri: &str,
		kind: SharedApplyKind,
		tx: Option<WireTx>,
	) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if entry.role != SharedStateRole::Owner || entry.needs_resync {
			return None;
		}

		let group_id = entry.current_undo_group;

		if entry.in_flight.is_some() {
			if kind == SharedApplyKind::Edit
				&& let Some(tx) = tx
			{
				entry.pending_deltas.push_back((tx, group_id));
			}
			return None;
		}

		entry.in_flight = Some(InFlightEdit {
			epoch: entry.epoch,
			base_seq: entry.seq,
		});

		Some(RequestPayload::SharedApply {
			uri: uri.to_string(),
			kind,
			epoch: entry.epoch,
			base_seq: entry.seq,
			base_hash64: entry.auth_hash64,
			base_len_chars: entry.auth_len_chars,
			tx,
			undo_group: group_id,
		})
	}

	/// Prepares a [`SharedApplyKind::Edit`] request.
	pub fn prepare_edit(
		&mut self,
		uri: &str,
		tx: &Transaction,
		new_group: bool,
	) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if new_group {
			entry.current_undo_group = entry.current_undo_group.wrapping_add(1).max(1);
		}

		let wire = convert::tx_to_wire(tx);
		self.prepare_apply(uri, SharedApplyKind::Edit, Some(wire))
	}

	/// Prepares a [`SharedApplyKind::Undo`] request.
	pub fn prepare_undo(&mut self, uri: &str) -> Option<RequestPayload> {
		self.prepare_apply(uri, SharedApplyKind::Undo, None)
	}

	/// Prepares a [`SharedApplyKind::Redo`] request.
	pub fn prepare_redo(&mut self, uri: &str) -> Option<RequestPayload> {
		self.prepare_apply(uri, SharedApplyKind::Redo, None)
	}

	/// Handles an application acknowledgment from the broker.
	///
	/// Clears the in-flight guard and advances the authoritative fingerprint.
	/// Returns the [`WireTx`] if the broker provided a result the client must apply.
	pub fn handle_apply_ack(
		&mut self,
		uri: &str,
		_kind: SharedApplyKind,
		epoch: SyncEpoch,
		seq: SyncSeq,
		applied_tx: Option<WireTx>,
		hash64: u64,
		len_chars: u64,
		_history_from: Option<u64>,
		_history_to: Option<u64>,
		_history_group: Option<u64>,
	) -> Option<WireTx> {
		let entry = self.docs.get_mut(uri)?;
		let in_flight = entry.in_flight?;

		let expected = in_flight.base_seq.0.wrapping_add(1);
		if epoch == in_flight.epoch && seq.0 == expected {
			entry.seq = seq;
			entry.auth_hash64 = hash64;
			entry.auth_len_chars = len_chars;
			entry.in_flight = None;
			return applied_tx;
		}

		tracing::warn!(
			?uri,
			"stale or mismatched SharedApplyAck ignored: got={epoch:?}/{seq:?}, expected={:?}/{}",
			in_flight.epoch,
			expected
		);
		None
	}

	/// Collects queued edit requests once the in-flight delta is acknowledged.
	pub fn drain_pending_edit_requests(&mut self) -> Vec<RequestPayload> {
		let mut out = Vec::new();

		for (uri, entry) in &mut self.docs {
			if entry.role == SharedStateRole::Owner
				&& !entry.needs_resync
				&& entry.in_flight.is_none()
				&& let Some((tx, gid)) = entry.pending_deltas.pop_front()
			{
				entry.in_flight = Some(InFlightEdit {
					epoch: entry.epoch,
					base_seq: entry.seq,
				});

				out.push(RequestPayload::SharedApply {
					uri: uri.clone(),
					kind: SharedApplyKind::Edit,
					epoch: entry.epoch,
					base_seq: entry.seq,
					base_hash64: entry.auth_hash64,
					base_len_chars: entry.auth_len_chars,
					tx: Some(tx),
					undo_group: gid,
				});
			}
		}

		out
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

	/// Returns true if the provided snapshot represents a state advanced from local tracking.
	fn snapshot_is_newer(entry: &SharedDocEntry, snap: &DocStateSnapshot) -> bool {
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

	/// Returns the local role for a document URI.
	pub fn role_for_uri(&self, uri: &str) -> Option<SharedStateRole> {
		self.docs.get(uri).map(|entry| entry.role)
	}

	/// Returns the current sync status for UI display.
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

	/// Disables all shared state tracking and clears document mappings.
	pub fn disable_all(&mut self) {
		self.docs.clear();
		self.uri_to_doc_id.clear();
		self.doc_id_to_uri.clear();
	}

	/// Caches view snapshots for a local edit group.
	pub fn cache_view_group(
		&mut self,
		uri: &str,
		group_id: u64,
		pre: HashMap<ViewId, ViewSnapshot>,
		post: HashMap<ViewId, ViewSnapshot>,
	) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry
				.view_history
				.groups
				.insert(group_id, GroupViewState { pre, post });
		}
	}

	/// Retrieves cached view state for a group.
	pub fn get_view_group(&self, uri: &str, group_id: u64) -> Option<&GroupViewState> {
		self.docs.get(uri)?.view_history.groups.get(&group_id)
	}

	/// Returns the current local undo group ID.
	pub fn current_undo_group(&self, uri: &str) -> u64 {
		self.docs
			.get(uri)
			.map(|e| e.current_undo_group)
			.unwrap_or(0)
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
			history_head_id: None,
			history_root_id: None,
			history_head_group: None,
		};

		let entry = &mut SharedDocEntry {
			doc_id: DocumentId(1),
			epoch: SyncEpoch(0),
			seq: SyncSeq(0),
			role: SharedStateRole::Follower,
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Unlocked,
			needs_resync: false,
			resync_requested: false,
			open_refcount: 1,
			pending_deltas: VecDeque::new(),
			in_flight: None,
			last_activity_sent: None,
			focus_seq: 0,
			next_nonce: 1,
			pending_align: None,
			auth_hash64: 0,
			auth_len_chars: 0,
			current_undo_group: 1,
			view_history: SharedViewHistory::default(),
		};

		SharedStateManager::apply_snapshot_state(entry, &snapshot, SessionId(1), true);
		assert!(!entry.needs_resync);
	}

	#[test]
	fn test_empty_snapshot_ignored_when_doc_not_empty() {
		let mut manager = SharedStateManager::new();
		let uri = "file:///test.rs";
		manager.prepare_open(uri, "hello", DocumentId(1));
		let entry = manager.docs.get_mut(uri).unwrap();
		entry.pending_align = Some(SyncNonce(1));

		let snapshot = DocStateSnapshot {
			uri: uri.to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(0),
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Unlocked,
			hash64: 999,
			len_chars: 5,
			history_head_id: None,
			history_root_id: None,
			history_head_group: None,
		};

		let text = manager.handle_snapshot_response(
			uri,
			snapshot,
			SyncNonce(1),
			"".to_string(),
			SessionId(1),
		);
		assert!(text.is_none());
	}

	#[test]
	fn test_empty_snapshot_applied_when_doc_empty() {
		let mut manager = SharedStateManager::new();
		let uri = "file:///test.rs";
		manager.prepare_open(uri, "hello", DocumentId(1));
		let entry = manager.docs.get_mut(uri).unwrap();
		entry.pending_align = Some(SyncNonce(1));

		let snapshot = DocStateSnapshot {
			uri: uri.to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(0),
			owner: None,
			preferred_owner: None,
			phase: DocSyncPhase::Unlocked,
			hash64: 0,
			len_chars: 0,
			history_head_id: None,
			history_root_id: None,
			history_head_group: None,
		};

		let text = manager.handle_snapshot_response(
			uri,
			snapshot,
			SyncNonce(1),
			"".to_string(),
			SessionId(1),
		);
		assert_eq!(text, Some("".to_string()));
	}

	#[test]
	fn test_repair_text_clears_owner_pipeline() {
		let mut manager = SharedStateManager::new();
		let uri = "file:///test.rs";
		let sid = SessionId(1);
		manager.prepare_open(uri, "hello", DocumentId(1));

		let entry = manager.docs.get_mut(uri).unwrap();
		entry.role = SharedStateRole::Owner;
		entry.owner = Some(sid);
		entry.in_flight = Some(InFlightEdit {
			epoch: SyncEpoch(1),
			base_seq: SyncSeq(0),
		});
		entry.pending_deltas.push_back((WireTx(Vec::new()), 1));
		entry.pending_align = Some(SyncNonce(1));

		let snapshot = DocStateSnapshot {
			uri: uri.to_string(),
			epoch: SyncEpoch(1),
			seq: SyncSeq(0),
			owner: Some(sid),
			preferred_owner: Some(sid),
			phase: DocSyncPhase::Owned,
			hash64: 123,
			len_chars: 10,
			history_head_id: None,
			history_root_id: None,
			history_head_group: None,
		};

		let text =
			manager.handle_focus_ack(snapshot, SyncNonce(1), Some("repaired".to_string()), sid);

		assert_eq!(text, Some("repaired".to_string()));

		let entry = manager.docs.get(uri).unwrap();
		assert!(entry.in_flight.is_none());
		assert!(entry.pending_deltas.is_empty());
		assert!(!entry.needs_resync);
	}
}
