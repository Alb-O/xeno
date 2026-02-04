//! Wire types for the Xeno broker IPC protocol.
//!
//! This module defines the core data structures used for communication between
//! editor sessions and the broker daemon, as well as between the broker and
//! LSP servers.

use serde::{Deserialize, Serialize};

/// Unique identifier for requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub u64);

/// Unique identifier for broker sessions (editor connections).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// Unique identifier for documents managed by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DocId(pub u64);

/// Monotonic ownership generation for a shared document.
///
/// Increments each time ownership changes hands for a given document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncEpoch(pub u64);

/// Monotonic edit sequence number within a single epoch.
///
/// Strictly increments per applied delta under the same epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncSeq(pub u64);

/// Nonce used to correlate resync-capable responses with the caller's local state.
///
/// Editor MUST ignore FocusAck/Snapshot responses whose nonce != the most recent
/// in-flight nonce for that URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncNonce(pub u64);

/// Phase of a shared document synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocSyncPhase {
	/// Document is owned and writable by the current owner.
	Owned,
	/// Document is unlocked and up-for-grabs (no owner).
	Unlocked,
	/// Document owner is blocked: MUST align (Focus/Resync) before publishing.
	Diverged,
}

/// Canonical snapshot of a shared document state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocStateSnapshot {
	/// Document URI.
	pub uri: String,
	/// Current ownership epoch.
	pub epoch: SyncEpoch,
	/// Current sequence number within the epoch.
	pub seq: SyncSeq,
	/// Current owner session ID, if any.
	pub owner: Option<SessionId>,
	/// Preferred owner session ID (focused editor).
	pub preferred_owner: Option<SessionId>,
	/// Current synchronization phase.
	pub phase: DocSyncPhase,
	/// Authoritative 64-bit hash of the content.
	pub hash64: u64,
	/// Authoritative length of the content in characters.
	pub len_chars: u64,
	/// Current history head node identifier.
	pub history_head_id: Option<u64>,
	/// History root node identifier.
	pub history_root_id: Option<u64>,
	/// Group identifier of the current history head.
	pub history_head_group: Option<u64>,
}

/// A single serializable edit operation for buffer sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireOp {
	/// Skip over N characters.
	Retain(usize),
	/// Delete N characters.
	Delete(usize),
	/// Insert the given string at the current position.
	Insert(String),
}

/// A serializable transaction: an ordered list of [`WireOp`]s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTx(pub Vec<WireOp>);

/// Unique identifier for LSP servers managed by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServerId(pub u64);

/// Kind of shared-state mutation being applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SharedApplyKind {
	/// Standard text edit.
	Edit,
	/// Undo last mutation.
	Undo,
	/// Redo previously undone mutation.
	Redo,
}

/// Classification of frames transmitted over the IPC socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcFrame {
	/// A request initiated by the editor.
	Request(Request),
	/// A response from the broker.
	Response(Response),
	/// An asynchronous event from the broker.
	Event(Event),
}

/// A request from the editor to the broker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
	/// Unique request identifier for correlation.
	pub id: RequestId,
	/// The request payload.
	pub payload: RequestPayload,
}

impl Request {
	/// Wraps a payload in a new request.
	#[must_use]
	pub fn new(payload: RequestPayload) -> Self {
		Self {
			id: RequestId(0),
			payload,
		}
	}
}

/// Request payload variants for broker operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestPayload {
	/// Connectivity check.
	Ping,
	/// Register session ID for event routing.
	Subscribe {
		/// The session identity.
		session_id: SessionId,
	},
	/// Start a new LSP server instance.
	LspStart {
		/// Server configuration.
		config: LspServerConfig,
	},
	/// Send a notification to an LSP server.
	LspSend {
		/// Originating session.
		session_id: SessionId,
		/// Target server.
		server_id: ServerId,
		/// Raw LSP JSON-RPC message.
		message: String,
	},
	/// Send a request to an LSP server and await response.
	LspRequest {
		/// Originating session.
		session_id: SessionId,
		/// Target server.
		server_id: ServerId,
		/// Raw LSP JSON-RPC message.
		message: String,
		/// Maximum time to wait for response.
		timeout_ms: Option<u64>,
	},
	/// Send a response to a server-initiated request.
	LspReply {
		/// Target server.
		server_id: ServerId,
		/// Raw LSP JSON-RPC message.
		message: String,
	},

	/// Open (or join) a shared document.
	SharedOpen {
		/// Document URI.
		uri: String,
		/// Initial text content (if creating).
		text: String,
		/// Optional version hint from the client.
		version_hint: Option<u32>,
	},
	/// Leave document synchronization.
	SharedClose {
		/// Document URI.
		uri: String,
	},

	/// Apply a shared-state mutation (Edit/Undo/Redo) under preconditions.
	///
	/// Preconditions enforce that the caller is the current preferred owner
	/// and that their local state is aligned with the broker's authoritative
	/// epoch, sequence, and fingerprint.
	SharedApply {
		/// Document URI.
		uri: String,
		/// Kind of mutation.
		kind: SharedApplyKind,
		/// Authoritative era this edit targets.
		epoch: SyncEpoch,
		/// Base sequence number this edit applies to.
		base_seq: SyncSeq,
		/// Content hash before applying this mutation.
		base_hash64: u64,
		/// Content length before applying this mutation.
		base_len_chars: u64,
		/// Transaction data (required for Edit, None for Undo/Redo).
		tx: Option<WireTx>,
		/// Undo group token for grouping multiple deltas into one user-level undo.
		undo_group: u64,
	},

	/// Record user activity to reset the idle timer.
	SharedActivity {
		/// Document URI.
		uri: String,
	},

	/// Update focus status with atomic ownership acquisition.
	///
	/// When `focused` is true, the broker attempts to grant ownership to the caller.
	/// If the client's fingerprint mismatches authoritative state, a repair text
	/// is returned in the acknowledgment.
	SharedFocus {
		/// Document URI.
		uri: String,
		/// Whether the session is currently focused.
		focused: bool,
		/// Monotonic focus transition sequence.
		focus_seq: u64,
		/// Nonce for correlating the response.
		nonce: SyncNonce,
		/// Client's current hash for alignment.
		client_hash64: Option<u64>,
		/// Client's current length for alignment.
		client_len_chars: Option<u64>,
	},

	/// Request a full document snapshot from the broker.
	SharedResync {
		/// Document URI.
		uri: String,
		/// Nonce for correlating the response.
		nonce: SyncNonce,
		/// Client's current hash.
		client_hash64: Option<u64>,
		/// Client's current length.
		client_len_chars: Option<u64>,
	},

	/// Perform a semantic search across indexed documents.
	KnowledgeSearch {
		/// Search query string.
		query: String,
		/// Maximum results to return.
		limit: u32,
	},
}

/// Configuration for an LSP server instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
	/// Path to the server executable.
	pub command: String,
	/// Command-line arguments.
	pub args: Vec<String>,
	/// Environment variables.
	pub env: Vec<(String, String)>,
	/// Working directory.
	pub cwd: Option<String>,
}

/// A response from the broker to the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
	/// Corresponding request identifier.
	pub request_id: RequestId,
	/// The response payload.
	pub payload: Option<ResponsePayload>,
	/// Protocol error code, if the request failed.
	pub error: Option<ErrorCode>,
}

/// Response payload variants for broker operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePayload {
	/// Reply to Ping.
	Pong,
	/// Confirmation of subscription.
	Subscribed,

	/// Confirmation that an LSP server was started.
	LspStarted {
		/// Assigned server identifier.
		server_id: ServerId,
	},
	/// Message received from an LSP server.
	LspMessage {
		/// Source server.
		server_id: ServerId,
		/// Raw LSP JSON-RPC message.
		message: String,
	},
	/// Confirmation that an LSP notification was queued.
	LspSent {
		/// Target server.
		server_id: ServerId,
	},

	/// Confirmation that a shared document was opened.
	SharedOpened {
		/// Current state snapshot.
		snapshot: DocStateSnapshot,
		/// Full text if joining an existing session.
		text: Option<String>,
	},
	/// Confirmation that a shared document was closed.
	SharedClosed,

	/// Unified acknowledgment for Edit/Undo/Redo.
	SharedApplyAck {
		/// Document URI.
		uri: String,
		/// Kind of mutation applied.
		kind: SharedApplyKind,
		/// New ownership epoch.
		epoch: SyncEpoch,
		/// New sequence number.
		seq: SyncSeq,
		/// The applied transaction (typically for Undo/Redo).
		applied_tx: Option<WireTx>,
		/// Authoritative hash after apply.
		hash64: u64,
		/// Authoritative length after apply.
		len_chars: u64,
		/// Previous history head node identifier.
		history_from_id: Option<u64>,
		/// New history head node identifier.
		history_to_id: Option<u64>,
		/// History group identifier affected by this operation.
		history_group: Option<u64>,
	},

	/// Acknowledgment of a focus update.
	SharedFocusAck {
		/// Echoed nonce from request.
		nonce: SyncNonce,
		/// Updated state snapshot.
		snapshot: DocStateSnapshot,
		/// Repair text if the client fingerprint was mismatched.
		repair_text: Option<String>,
	},

	/// Full document snapshot delivered for resync.
	SharedSnapshot {
		/// Echoed nonce from request.
		nonce: SyncNonce,
		/// authoritative full text.
		text: String,
		/// autoritative state metadata.
		snapshot: DocStateSnapshot,
	},

	/// Confirmation of activity record.
	SharedActivityAck,

	/// Result set for a knowledge search query.
	KnowledgeSearchResults {
		/// Matching document chunks.
		hits: Vec<KnowledgeHit>,
	},
}

/// Search hit for a knowledge query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeHit {
	/// source document URI.
	pub uri: String,
	/// Start offset in characters.
	pub start_char: u64,
	/// End offset in characters.
	pub end_char: u64,
	/// Relevance score.
	pub score: f64,
	/// Text preview fragment.
	pub preview: String,
}

/// Error codes for broker protocol operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
	/// Unspecified internal broker error.
	Internal,
	/// Request variant not recognized.
	UnknownRequest,
	/// Payload contains malformed or missing arguments.
	InvalidArgs,
	/// The specified LSP server is not running.
	ServerNotFound,
	/// Too many requests from this session.
	RateLimited,
	/// Authentication failed or session ID mismatch.
	AuthFailed,
	/// Feature not supported in this broker build.
	NotImplemented,
	/// Operation timed out before completion.
	Timeout,
	/// The referenced request ID was not found.
	RequestNotFound,

	/// Edit rejected: session is not the current writer.
	NotPreferredOwner,
	/// The document URI is not open in this broker.
	DocNotOpen,

	/// Edit rejected: sequence number mismatch (stale base).
	SyncSeqMismatch,
	/// Edit rejected: ownership epoch mismatch.
	SyncEpochMismatch,

	/// Edit rejected: content fingerprint mismatch (diverged).
	SyncFingerprintMismatch,

	/// The document was not found in the sync manager.
	SyncDocNotFound,
	/// The provided WireTx is malformed or violates constraints.
	InvalidDelta,
	/// Owner is blocked until alignment (Focus/Resync).
	OwnerNeedsResync,
	/// History stack is empty.
	NothingToUndo,
	/// Redo stack is empty.
	NothingToRedo,
	/// History is unavailable for this document (e.g. storage disabled).
	HistoryUnavailable,
}

/// Asynchronous async event from broker to editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
	/// Periodic heartbeat to keep connection alive.
	Heartbeat,

	/// Diagnostics published by an LSP server.
	LspDiagnostics {
		/// Source server.
		server_id: ServerId,
		/// Local document ID, if known.
		doc_id: Option<DocId>,
		/// Document URI.
		uri: String,
		/// Document version.
		version: Option<u32>,
		/// Raw LSP diagnostic payload.
		diagnostics: String,
	},
	/// Status change for an LSP server.
	LspStatus {
		/// Source server.
		server_id: ServerId,
		/// New status.
		status: LspServerStatus,
	},
	/// Notification from an LSP server.
	LspMessage {
		/// Source server.
		server_id: ServerId,
		/// Raw LSP JSON-RPC message.
		message: String,
	},
	/// Request from an LSP server.
	LspRequest {
		/// Source server.
		server_id: ServerId,
		/// Raw LSP JSON-RPC message.
		message: String,
	},

	/// delta broadcast to followers of a shared document.
	SharedDelta {
		/// Document URI.
		uri: String,
		/// Authority era.
		epoch: SyncEpoch,
		/// New authoritative sequence.
		seq: SyncSeq,
		/// Kind of mutation applied.
		kind: SharedApplyKind,
		/// The delta transaction.
		tx: WireTx,
		/// Session that originated the edit.
		origin: SessionId,
		/// Post-apply content hash.
		hash64: u64,
		/// Post-apply content length.
		len_chars: u64,
		/// Previous history head node identifier.
		history_from_id: Option<u64>,
		/// New history head node identifier.
		history_to_id: Option<u64>,
		/// History group identifier affected by this operation.
		history_group: Option<u64>,
	},

	/// authority has changed for a document.
	SharedOwnerChanged {
		/// New authoritative state.
		snapshot: DocStateSnapshot,
	},
	/// focus has changed for a document.
	SharedPreferredOwnerChanged {
		/// New authoritative state.
		snapshot: DocStateSnapshot,
	},
	/// Ownership released for a document.
	SharedUnlocked {
		/// New authoritative state.
		snapshot: DocStateSnapshot,
	},
}

/// Operational status of an LSP server instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspServerStatus {
	/// Server is initializing.
	Starting,
	/// Server is active and responding.
	Running,
	/// Server exited gracefully.
	Stopped,
	/// Server terminated unexpectedly.
	Crashed,
}
