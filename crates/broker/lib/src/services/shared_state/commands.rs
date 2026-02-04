use ropey::Rope;
use tokio::sync::oneshot;
use xeno_broker_proto::types::{
	ErrorCode, ResponsePayload, SessionId, SharedApplyKind, SyncEpoch, SyncNonce, SyncSeq, WireTx,
};

/// Commands for the shared state service actor.
#[derive(Debug)]
pub enum SharedStateCmd {
	/// Open a document or join an existing session.
	Open {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Initial text content (if creating).
		text: String,
		/// Optional version hint from the client.
		version_hint: Option<u32>,
		/// Reply channel for the opened state.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Decrement reference count for a session on a document.
	Close {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for confirmation.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Apply a shared-state mutation (Edit/Undo/Redo) under preconditions.
	///
	/// Preconditions enforce that the caller is the current preferred owner
	/// and that their local state is aligned with the broker's authoritative
	/// epoch, sequence, and fingerprint.
	Apply {
		/// The session identity (must be owner).
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Kind of mutation.
		kind: SharedApplyKind,
		/// Current ownership era.
		epoch: SyncEpoch,
		/// Base sequence number this delta applies to.
		base_seq: SyncSeq,
		/// Base hash precondition.
		base_hash64: u64,
		/// Base length precondition.
		base_len_chars: u64,
		/// The transaction data (required for Edit, None for Undo/Redo).
		tx: Option<WireTx>,
		/// Undo group identifier for the mutation.
		undo_group: u64,
		/// Reply channel for the acknowledgment.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Update activity timestamp for a document to prevent idle unlock.
	Activity {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for acknowledgment.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Update focus status for a document with atomic ownership acquisition.
	///
	/// When `focused` is true, the broker attempts to grant ownership to the caller.
	/// If the client's fingerprint mismatches authoritative state, a repair text
	/// is returned in the acknowledgment.
	Focus {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Whether the session is focused on the document.
		focused: bool,
		/// Monotonic sequence number for focus transitions.
		focus_seq: u64,
		/// Nonce for correlating the response.
		nonce: SyncNonce,
		/// Client's current hash for alignment check.
		client_hash64: Option<u64>,
		/// Client's current length for alignment check.
		client_len_chars: Option<u64>,
		/// Reply channel for the updated snapshot and optional repair text.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Fetch a full snapshot of the authoritative document.
	Resync {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Nonce for correlating the response.
		nonce: SyncNonce,
		/// Optional hash of the client's current content.
		client_hash64: Option<u64>,
		/// Optional length of the client's current content.
		client_len_chars: Option<u64>,
		/// Reply channel for the full snapshot.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Signal that a session has disconnected unexpectedly.
	SessionLost {
		/// The lost session identity.
		sid: SessionId,
	},
	/// Internal request for a document snapshot triad (epoch, seq, rope).
	Snapshot {
		/// Canonical document URI.
		uri: String,
		/// Reply channel for the triad.
		reply: oneshot::Sender<Option<(SyncEpoch, SyncSeq, Rope)>>,
	},
	/// Verifies if a document is currently active in the broker.
	IsOpen {
		/// Canonical document URI.
		uri: String,
		/// Reply channel for existence check.
		reply: oneshot::Sender<bool>,
	},
}
