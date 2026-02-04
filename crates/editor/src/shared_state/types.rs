//! Type definitions for shared document state synchronization.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use xeno_broker_proto::types::{DocSyncPhase, SessionId, SyncEpoch, SyncNonce, SyncSeq, WireTx};

use crate::buffer::{DocumentId, ViewId};
use crate::types::ViewSnapshot;

/// Minimum interval between sending activity heartbeats to the broker.
pub(super) const ACTIVITY_THROTTLE: Duration = Duration::from_millis(750);

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
		kind: xeno_broker_proto::types::SharedApplyKind,
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
		snapshot: xeno_broker_proto::types::DocStateSnapshot,
	},
	/// Preferred owner of a document changed.
	PreferredOwnerChanged {
		/// Canonical snapshot of the document state.
		snapshot: xeno_broker_proto::types::DocStateSnapshot,
	},
	/// Document ownership released (no current owner).
	Unlocked {
		/// Canonical snapshot of the document state.
		snapshot: xeno_broker_proto::types::DocStateSnapshot,
	},
	/// Broker responded to a SharedOpen request.
	Opened {
		/// Canonical snapshot of the document state.
		snapshot: xeno_broker_proto::types::DocStateSnapshot,
		/// Snapshot text if joining as follower.
		text: Option<String>,
	},
	/// Broker acknowledged an application (Edit/Undo/Redo).
	ApplyAck {
		/// Document URI.
		uri: String,
		/// Kind of mutation.
		kind: xeno_broker_proto::types::SharedApplyKind,
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
		snapshot: xeno_broker_proto::types::DocStateSnapshot,
	},
	/// Focus request acknowledged with updated snapshot.
	FocusAck {
		/// Nonce echoed from request.
		nonce: SyncNonce,
		/// Canonical snapshot of the document state.
		snapshot: xeno_broker_proto::types::DocStateSnapshot,
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

/// In-flight edit tracking for pipelining.
#[derive(Debug, Clone, Copy)]
pub(super) struct InFlightEdit {
	pub epoch: SyncEpoch,
	pub base_seq: SyncSeq,
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
pub(super) struct SharedDocEntry {
	pub doc_id: DocumentId,
	pub epoch: SyncEpoch,
	pub seq: SyncSeq,
	pub role: SharedStateRole,
	pub owner: Option<SessionId>,
	pub preferred_owner: Option<SessionId>,
	pub phase: DocSyncPhase,
	pub needs_resync: bool,
	pub resync_requested: bool,
	pub open_refcount: u32,
	pub pending_deltas: VecDeque<(WireTx, u64)>,
	pub in_flight: Option<InFlightEdit>,
	pub last_activity_sent: Option<Instant>,
	pub focus_seq: u64,
	pub next_nonce: u64,
	pub pending_align: Option<SyncNonce>,

	/// Authoritative fingerprint for current (epoch, seq).
	pub auth_hash64: u64,
	/// Authoritative length for current (epoch, seq).
	pub auth_len_chars: u64,

	/// Current local undo group identifier.
	pub current_undo_group: u64,
	/// View history cache for group-level undo/redo.
	pub view_history: SharedViewHistory,
}

impl SharedDocEntry {
	/// Returns true if mutations are currently prohibited due to role or divergence.
	pub fn is_blocked(&self) -> bool {
		self.role != SharedStateRole::Owner || self.needs_resync
	}
}
