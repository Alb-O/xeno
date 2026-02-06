//! LSP transport abstraction for pluggable communication backends.

use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use crate::client::LanguageServerId;
use crate::{AnyNotification, AnyRequest, AnyResponse, JsonValue, Message, ResponseError};

/// Events emitted by the transport layer to the LSP manager.
#[derive(Debug, Clone)]
pub enum TransportEvent {
	/// Server lifecycle status change.
	Status {
		/// Internal identifier for the server.
		server: LanguageServerId,
		/// New lifecycle status.
		status: TransportStatus,
	},
	/// Inbound message from the server (notification or request).
	Message {
		/// Internal identifier for the server.
		server: LanguageServerId,
		/// The message payload.
		message: Message,
	},
	/// Structured diagnostics event from the server.
	Diagnostics {
		/// Internal identifier for the server.
		server: LanguageServerId,
		/// Document URI.
		uri: String,
		/// Document version from LSP server's publishDiagnostics payload.
		///
		/// Optional because the LSP protocol does not require servers
		/// to include a version field in diagnostic notifications.
		version: Option<u32>,
		/// Diagnostics payload (JSON array).
		diagnostics: JsonValue,
	},
	/// The transport backend has disconnected.
	Disconnected,
}

/// Lifecycle status of an LSP server process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportStatus {
	/// Server is starting up.
	Starting,
	/// Server is running and responding to messages.
	Running,
	/// Server has stopped gracefully.
	Stopped,
	/// Server has crashed or terminated unexpectedly.
	Crashed,
}

/// Handle for a successfully started/acquired server.
#[derive(Debug, Clone, Copy)]
pub struct StartedServer {
	/// Internal identifier for the server.
	pub id: LanguageServerId,
}

/// Pluggable transport for LSP communication.
///
/// Abstracts server process management and JSON-RPC message routing.
#[async_trait]
pub trait LspTransport: Send + Sync {
	/// Returns a receiver for asynchronous events from the transport.
	fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent>;

	/// Starts or acquires a language server for the given configuration.
	async fn start(&self, cfg: crate::client::ServerConfig) -> crate::Result<StartedServer>;

	/// Sends an asynchronous notification to the server.
	async fn notify(&self, server: LanguageServerId, notif: AnyNotification) -> crate::Result<()>;

	/// Sends a notification and returns a receiver that fires when the message
	/// has been written to the underlying transport.
	async fn notify_with_barrier(
		&self,
		server: LanguageServerId,
		notif: AnyNotification,
	) -> crate::Result<oneshot::Receiver<()>>;

	/// Sends a synchronous request to the server and awaits its response.
	async fn request(
		&self,
		server: LanguageServerId,
		req: AnyRequest,
		timeout: Option<Duration>,
	) -> crate::Result<AnyResponse>;

	/// Replies to a request initiated by the server.
	async fn reply(
		&self,
		server: LanguageServerId,
		id: crate::types::RequestId,
		resp: Result<JsonValue, ResponseError>,
	) -> crate::Result<()>;

	/// Stops a language server process.
	async fn stop(&self, server: LanguageServerId) -> crate::Result<()>;
}
