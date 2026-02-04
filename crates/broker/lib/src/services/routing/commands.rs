use std::time::Duration;

use tokio::sync::oneshot;
use xeno_broker_proto::types::{ErrorCode, LspServerConfig, ServerId, SessionId};
use xeno_lsp::{AnyRequest, AnyResponse};

/// Commands for the routing service actor.
#[derive(Debug)]
pub enum RoutingCmd {
	/// Start or attach to an LSP server.
	LspStart {
		/// The session identity.
		sid: SessionId,
		/// The server process configuration.
		config: LspServerConfig,
		/// Reply channel for the server identity.
		reply: oneshot::Sender<Result<ServerId, ErrorCode>>,
	},
	/// Atomic operation to pin a leader, register a pending request, and transmit.
	BeginS2c {
		/// Target server.
		server_id: ServerId,
		/// ID assigned by the LSP server.
		request_id: xeno_lsp::RequestId,
		/// JSON payload.
		json: String,
		/// Channel for delivering the eventual editor reply.
		tx: oneshot::Sender<crate::core::LspReplyResult>,
		/// Immediate reply channel for transmission status.
		reply: oneshot::Sender<Result<(), xeno_lsp::ResponseError>>,
	},
	/// Complete a server-to-client request with a response.
	CompleteS2c {
		/// The replying session.
		sid: SessionId,
		/// Source server.
		server_id: ServerId,
		/// Original request ID.
		request_id: xeno_lsp::RequestId,
		/// The response payload or error.
		result: crate::core::LspReplyResult,
		/// Reply channel for status.
		reply: oneshot::Sender<bool>,
	},
	/// Cancel a pending server-to-client request.
	CancelS2c {
		/// Source server.
		server_id: ServerId,
		/// Original request ID.
		request_id: xeno_lsp::RequestId,
	},
	/// Initiate an editor-to-server request.
	BeginC2s {
		/// The originating session.
		sid: SessionId,
		/// Target server.
		server_id: ServerId,
		/// The LSP request payload.
		req: AnyRequest,
		/// Maximum time to wait for a reply.
		timeout: Duration,
		/// Reply channel for the server response.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Internal result of a client-to-server request.
	C2sResp {
		/// Source server.
		server_id: ServerId,
		/// The response payload.
		resp: AnyResponse,
		/// Reply channel for the origin.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Internal signal that a client-to-server request timed out.
	C2sTimeout {
		/// Source server.
		server_id: ServerId,
		/// The wire ID assigned by the broker.
		wire_id: xeno_lsp::RequestId,
		/// Reply channel for the origin.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Internal signal that transmitting a request to the LSP server failed.
	C2sSendFailed {
		/// Source server.
		server_id: ServerId,
		/// The wire ID assigned by the broker.
		wire_id: xeno_lsp::RequestId,
		/// Reply channel for the origin.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Authoritative signal that a session has disconnected.
	SessionLost {
		/// The lost session identity.
		sid: SessionId,
	},
	/// Signal that an LSP process has exited or crashed.
	ServerExited {
		/// The exited server.
		server_id: ServerId,
		/// Whether the process returned a non-zero exit code.
		crashed: bool,
	},
	/// Signal that an idle lease has expired.
	LeaseExpired {
		/// The idle server.
		server_id: ServerId,
		/// The era this lease was scheduled in.
		generation: u64,
	},
	/// Transmit a notification to an LSP server.
	LspSendNotif {
		/// Originating session.
		sid: SessionId,
		/// Target server.
		server_id: ServerId,
		/// JSON message content.
		message: String,
		/// Reply channel for confirmation.
		reply: oneshot::Sender<Result<(), ErrorCode>>,
	},
	/// Handle an inbound notification from an LSP server.
	ServerNotif {
		/// Source server.
		server_id: ServerId,
		/// JSON message content.
		message: String,
	},
	/// Update broker-owned LSP document state from buffer sync (initial open).
	LspDocOpen {
		/// Canonical document URI.
		uri: String,
		/// Full document text.
		text: String,
	},
	/// Update broker-owned LSP document state from buffer sync (content change).
	LspDocUpdate {
		/// Canonical document URI.
		uri: String,
		/// Full document text.
		text: String,
	},
	/// Close a broker-owned LSP document (no active sessions).
	LspDocClose {
		/// Canonical document URI.
		uri: String,
	},
	/// Terminate all managed processes and shutdown the service.
	TerminateAll,
}
