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

/// Monotonic ownership generation for a buffer sync URI.
///
/// Increments each time ownership changes hands for a given document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncEpoch(pub u64);

/// Monotonic edit sequence number within a single epoch.
///
/// Strictly increments per applied delta under the same epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncSeq(pub u64);

/// Phase of a buffer sync document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocSyncPhase {
	/// Document is owned and writable by the current owner.
	Owned,
	/// Document is unlocked and up-for-grabs (no owner).
	Unlocked,
	/// Document owner must resync before publishing deltas.
	Diverged,
}

/// Canonical snapshot of a buffer sync document state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocStateSnapshot {
	/// Canonical URI for the document.
	pub uri: String,
	/// Current ownership epoch.
	pub epoch: SyncEpoch,
	/// Current sequence number.
	pub seq: SyncSeq,
	/// Current owner session (if any).
	pub owner: Option<SessionId>,
	/// Current phase of the document.
	pub phase: DocSyncPhase,
	/// 64-bit hash of the authoritative document content.
	pub hash64: u64,
	/// Length of the authoritative document in characters.
	pub len_chars: u64,
}

/// A single serializable edit operation for buffer sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireOp {
	/// Retain the next `n` characters unchanged.
	Retain(usize),
	/// Delete the next `n` characters.
	Delete(usize),
	/// Insert the given UTF-8 text.
	Insert(String),
}

/// A serializable transaction: an ordered list of [`WireOp`]s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTx(pub Vec<WireOp>);

/// Role of a session in a buffer sync document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferSyncRole {
	/// Session owns the document and may submit deltas.
	Owner,
	/// Session is a live follower (read-only).
	Follower,
}

/// Unique identifier for LSP servers managed by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServerId(pub u64);

/// Classification of frames transmitted over the IPC socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcFrame {
	/// A request from editor to broker.
	Request(Request),
	/// A response from broker to editor.
	Response(Response),
	/// An async event from broker to editor.
	Event(Event),
}

/// A request from the editor to the broker.
///
/// The `id` field is automatically managed and overwritten by the RPC mainloop
/// during transmission. When constructing a new request, use [`Request::new`]
/// which sets a placeholder value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
	/// Unique identifier for this request.
	pub id: RequestId,
	/// The request payload.
	pub payload: RequestPayload,
}

impl Request {
	/// Create a new request with a placeholder ID.
	#[must_use]
	pub fn new(payload: RequestPayload) -> Self {
		Self {
			id: RequestId(0),
			payload,
		}
	}
}

/// Request payload variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestPayload {
	/// Simple ping for connectivity check.
	Ping,
	/// Subscribe to async events from the broker.
	Subscribe {
		/// Session ID for this connection.
		session_id: SessionId,
	},
	/// Start an LSP server for a project.
	LspStart {
		/// Configuration for the LSP server.
		config: LspServerConfig,
	},
	/// Send a notification or request to an LSP server.
	LspSend {
		/// Session ID originating this message.
		///
		/// Enforces document ownership for text synchronization.
		session_id: SessionId,
		/// Target LSP server.
		server_id: ServerId,
		/// The LSP message (JSON-RPC string).
		message: String,
	},
	/// Send a request to an LSP server and wait for the response.
	LspRequest {
		/// Session ID originating this request.
		session_id: SessionId,
		/// Target LSP server.
		server_id: ServerId,
		/// The LSP request (JSON-RPC string).
		message: String,
		/// Optional timeout in milliseconds.
		timeout_ms: Option<u64>,
	},
	/// Reply to a request initiated by an LSP server.
	LspReply {
		/// Target LSP server.
		server_id: ServerId,
		/// The LSP response (JSON-RPC string).
		message: String,
	},
	/// Open (or join) a buffer sync document.
	BufferSyncOpen {
		/// Canonical URI for the document.
		uri: String,
		/// Full text content when opening.
		text: String,
		/// Optional version hint from the local document.
		version_hint: Option<u32>,
	},
	/// Close a buffer sync document.
	BufferSyncClose {
		/// Canonical URI for the document.
		uri: String,
	},
	/// Submit an edit delta to a buffer sync document.
	BufferSyncDelta {
		/// Canonical URI for the document.
		uri: String,
		/// Expected ownership epoch.
		epoch: SyncEpoch,
		/// Expected base sequence number.
		base_seq: SyncSeq,
		/// The edit transaction.
		tx: WireTx,
	},
	/// Notify broker of local activity for a buffer sync document.
	BufferSyncActivity {
		/// Canonical URI for the document.
		uri: String,
	},
	/// Request ownership of a buffer sync document.
	BufferSyncTakeOwnership {
		/// Canonical URI for the document.
		uri: String,
	},
	/// Release ownership of a buffer sync document.
	BufferSyncReleaseOwnership {
		/// Canonical URI for the document.
		uri: String,
	},
	/// Confirm ownership of a buffer sync document.
	BufferSyncOwnerConfirm {
		/// Canonical URI for the document.
		uri: String,
		/// Expected ownership epoch.
		epoch: SyncEpoch,
		/// Length of the document in characters.
		len_chars: u64,
		/// 64-bit hash of the document content.
		hash64: u64,
		/// Allow mismatch when optimistic edits are queued.
		allow_mismatch: bool,
	},
	/// Request a full resync snapshot from the broker.
	BufferSyncResync {
		/// Canonical URI for the document.
		uri: String,
	},
	/// Query the broker knowledge index.
	KnowledgeSearch {
		/// Full-text search query.
		query: String,
		/// Maximum number of hits to return.
		limit: u32,
	},
}

/// Configuration for an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
	/// The command to execute.
	pub command: String,
	/// Arguments for the command.
	pub args: Vec<String>,
	/// Environment variables to set.
	pub env: Vec<(String, String)>,
	/// Working directory.
	pub cwd: Option<String>,
}

/// A response from the broker to the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
	/// The request this responds to.
	pub request_id: RequestId,
	/// The response payload when successful.
	pub payload: Option<ResponsePayload>,
	/// The error code when the request failed.
	pub error: Option<ErrorCode>,
}

/// Response payload variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePayload {
	/// Simple pong response.
	Pong,
	/// Subscription acknowledged.
	Subscribed,
	/// LSP server started.
	LspStarted {
		/// The server ID assigned.
		server_id: ServerId,
	},
	/// LSP message received from server.
	LspMessage {
		/// Source server.
		server_id: ServerId,
		/// The LSP message (JSON-RPC string).
		message: String,
	},
	/// Message sent to LSP server.
	LspSent {
		/// Target server.
		server_id: ServerId,
	},
	/// Buffer sync document opened successfully.
	BufferSyncOpened {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
		/// Full text snapshot (present when joining as follower).
		text: Option<String>,
	},
	/// Buffer sync document closed successfully.
	BufferSyncClosed,
	/// Buffer sync delta acknowledged by broker.
	BufferSyncDeltaAck {
		/// New sequence number after applying the delta.
		seq: SyncSeq,
	},
	/// Buffer sync ownership transferred.
	BufferSyncOwnership {
		/// Status of the ownership request.
		status: BufferSyncOwnershipStatus,
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Buffer sync ownership released.
	BufferSyncReleased {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Result of an ownership confirmation.
	BufferSyncOwnerConfirmResult {
		/// Status of the confirmation.
		status: BufferSyncOwnerConfirmStatus,
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
		/// Full text snapshot (present when status is NeedSnapshot).
		text: Option<String>,
	},
	/// Buffer sync full snapshot for resync.
	BufferSyncSnapshot {
		/// Full text content.
		text: String,
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Buffer sync activity acknowledged.
	BufferSyncActivityAck,
	/// Knowledge search results.
	KnowledgeSearchResults {
		/// Ranked knowledge hits.
		hits: Vec<KnowledgeHit>,
	},
}

/// Search hit for a knowledge query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeHit {
	/// Canonical URI for the document.
	pub uri: String,
	/// Start offset (character index) of the hit.
	pub start_char: u64,
	/// End offset (character index) of the hit.
	pub end_char: u64,
	/// BM25 relevance score.
	pub score: f64,
	/// Preview text snippet for display.
	pub preview: String,
}

/// Error codes for broker operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
	/// Generic internal error.
	Internal,
	/// Unknown request type.
	UnknownRequest,
	/// Invalid arguments.
	InvalidArgs,
	/// Server not found.
	ServerNotFound,
	/// Rate limited.
	RateLimited,
	/// Authentication failed.
	AuthFailed,
	/// Feature not implemented.
	NotImplemented,
	/// Request timed out.
	Timeout,
	/// Request not found (e.g., reply to non-existent or already-cancelled request).
	RequestNotFound,
	/// Document is not owned by this session.
	///
	/// Returned when a session attempts to send text synchronization notifications
	/// (didOpen, didChange, didClose) for a document currently owned by another session.
	NotDocOwner,
	/// Document is not open.
	///
	/// Returned when an operation requires `textDocument/didOpen` to have been called first.
	DocNotOpen,
	/// Buffer sync sequence number mismatch.
	///
	/// The submitted delta's `base_seq` does not match the broker's current sequence.
	/// The client should request a resync.
	SyncSeqMismatch,
	/// Buffer sync epoch mismatch.
	///
	/// The submitted delta targets a stale ownership epoch.
	SyncEpochMismatch,
	/// Buffer sync document not found.
	///
	/// The URI has no active sync document entry.
	SyncDocNotFound,
	/// Buffer sync delta is malformed or out of bounds.
	InvalidDelta,
	/// Owner must resync before publishing deltas.
	OwnerNeedsResync,
}

/// Async event from broker to editor (no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
	/// Periodic heartbeat.
	Heartbeat,
	/// LSP diagnostics received.
	LspDiagnostics {
		/// Source server.
		server_id: ServerId,
		/// Target document ID (if known to broker).
		///
		/// Optional because diagnostics may arrive before the document
		/// is registered via `didOpen`, or after all sessions close it.
		doc_id: Option<DocId>,
		/// Document URI.
		uri: String,
		/// Document version from LSP server's publishDiagnostics payload.
		///
		/// Optional because the LSP protocol does not require servers
		/// to include a version field in diagnostic notifications.
		version: Option<u32>,
		/// Diagnostics (serialized JSON).
		diagnostics: String,
	},
	/// LSP server status changed.
	LspStatus {
		/// The LSP server.
		server_id: ServerId,
		/// New status.
		status: LspServerStatus,
	},
	/// LSP message received from server (asynchronously).
	LspMessage {
		/// Source server.
		server_id: ServerId,
		/// The LSP message (JSON-RPC string).
		message: String,
	},
	/// LSP request received from server (requires response via LspReply).
	LspRequest {
		/// Source server.
		server_id: ServerId,
		/// The LSP request (JSON-RPC string).
		message: String,
	},
	/// A buffer sync delta broadcast from the broker.
	BufferSyncDelta {
		/// Document URI.
		uri: String,
		/// Ownership epoch.
		epoch: SyncEpoch,
		/// New sequence number after this delta.
		seq: SyncSeq,
		/// The edit transaction.
		tx: WireTx,
	},
	/// Buffer sync ownership changed.
	BufferSyncOwnerChanged {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
	/// Buffer sync document unlocked ("up-for-grabs" with no current owner).
	BufferSyncUnlocked {
		/// Canonical snapshot of the document state.
		snapshot: DocStateSnapshot,
	},
}

/// Status of a buffer sync ownership request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferSyncOwnershipStatus {
	/// Ownership granted.
	Granted,
	/// Ownership denied (e.g. another session already owns it).
	Denied,
	/// Session is already the owner.
	AlreadyOwner,
}

/// Status of a buffer sync ownership confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferSyncOwnerConfirmStatus {
	/// Local content matches broker; ownership confirmed.
	Confirmed,
	/// Local content mismatch; snapshot required.
	NeedSnapshot,
}

/// Status of an LSP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspServerStatus {
	/// Server is starting up.
	Starting,
	/// Server is running and ready.
	Running,
	/// Server has stopped.
	Stopped,
	/// Server crashed.
	Crashed,
}
