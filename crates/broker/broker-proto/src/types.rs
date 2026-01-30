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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// Unique identifier for documents managed by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocId(pub u64);

/// Unique identifier for LSP servers managed by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServerId(pub u64);

/// A single IPC frame between editor and broker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcFrame {
	/// A request from editor to broker.
	Request(Request),
	/// A response from broker to editor.
	Response(Response),
	/// An async event from broker to editor (no response expected).
	Event(Event),
}

/// A request from the editor to the broker.
///
/// # Note
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
	/// Start an LSP server.
	LspStart {
		/// Configuration for the LSP server.
		config: LspServerConfig,
	},
	/// Send a message to an LSP server.
	LspSend {
		/// Target LSP server.
		server_id: ServerId,
		/// The LSP message (JSON-RPC string).
		message: String,
	},
	/// Send a request to an LSP server and wait for the response.
	LspRequest {
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
		/// Target document.
		doc_id: DocId,
		/// Document URI.
		uri: String,
		/// Document version.
		version: u32,
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
