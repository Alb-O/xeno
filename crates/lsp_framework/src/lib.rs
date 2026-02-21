//! Asynchronous [Language Server Protocol (LSP)][lsp] framework based on [tower].
//!
//! [lsp]: https://microsoft.github.io/language-server-protocol/overviews/lsp/overview/
//! [tower]: https://github.com/tower-rs/tower
//!
//! Internal crate — use [`xeno_lsp`] for editor/client integration.
//!
//! This crate provides the tower-based LSP service framework: the core
//! [`LspService`] trait, the [`MainLoop`] driver, and pluggable middleware:
//!
//! * [`concurrency::Concurrency`]: Incoming request multiplexing and cancellation.
//! * [`panic::CatchUnwind`]: Turn panics into errors.
//! * [`tracing::Tracing`]: Logger spans with methods instrumenting handlers.
//! * [`server::Lifecycle`]: Server initialization, shutting down, and exit handling.
//! * [`router::Router`]: "Root" service to dispatch requests, notifications and events.
//!
//! ## Cargo features
//!
//! No features are enabled by default.
//!
//! * `client-monitor`: Client process monitor middleware.
//! * `omni-trait`: `LanguageServer` and `LanguageClient` mega-traits.
//! * `stdio`: Pipe-based stdin/stdout communication channel (unix-only).
//! * `forward`: Impl `LspService` for `{Client,Server}Socket`.
//! * `tokio`: Tokio-compat helpers in `stdio`.
//! * `position`: Ropey-based position conversion and change computation.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
use std::io;
use std::ops::ControlFlow;

/// Re-export of the [`lsp_types`] dependency of this crate.
pub use lsp_types;
pub use serde_json::Value as JsonValue;
use tower_service::Service;

mod event;
#[macro_use]
mod mainloop;
mod message;
mod protocol;
mod socket;
mod types;

pub use event::AnyEvent;
pub use mainloop::MainLoop;
pub use message::Message;
pub use protocol::JsonRpcProtocol;
pub use socket::{ClientSocket, ServerSocket};
pub use types::{AnyNotification, AnyRequest, AnyResponse, ErrorCode, RequestId, ResponseError};

mod concurrency;
mod panic;
mod router;
mod server;

/// Service forwarding implementations (requires `forward` feature).
#[cfg(feature = "forward")]
#[cfg_attr(docsrs, doc(cfg(feature = "forward")))]
mod forward;

#[cfg(feature = "client-monitor")]
#[cfg_attr(docsrs, doc(cfg(feature = "client-monitor")))]
mod client_monitor;

#[cfg(all(feature = "stdio", unix))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "stdio", unix))))]
mod stdio;

mod tracing;

/// Mega-traits for Language Server and Client implementations.
#[cfg(feature = "omni-trait")]
mod omni_trait;
#[cfg(feature = "omni-trait")]
#[cfg_attr(docsrs, doc(cfg(feature = "omni-trait")))]
pub use omni_trait::{LanguageClient, LanguageServer};

mod encoding;

pub use encoding::OffsetEncoding;

#[cfg(feature = "position")]
mod position;
#[cfg(feature = "position")]
pub use position::{char_range_to_lsp_range, char_to_lsp_position, lsp_position_to_char, lsp_range_to_char_range};

#[cfg(feature = "position")]
mod changes;
#[cfg(feature = "position")]
pub use changes::{IncrementalResult, compute_lsp_changes};

/// A convenient type alias for `Result` with `E` = [`enum@crate::Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Possible errors.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
	/// The service main loop stopped.
	#[error("service stopped")]
	ServiceStopped,
	/// The peer replies undecodable or invalid responses.
	#[error("deserialization failed: {0}")]
	Deserialize(String),
	/// The peer replies an error.
	#[error("{0}")]
	Response(#[from] ResponseError),
	/// The request timed out.
	#[error("request timed out: {0}")]
	RequestTimeout(String),
	/// The peer violates the Language Server Protocol.
	#[error("protocol error: {0}")]
	Protocol(String),
	/// Input/output errors from the underlying channels.
	#[error("{0}")]
	Io(String),
	/// The underlying channel reached EOF (end of file).
	#[error("the underlying channel reached EOF")]
	Eof,
	/// No handlers for events or mandatory notifications (not starting with `$/`).
	///
	/// Will not occur when catch-all handlers ([`router::Router::unhandled_event`] and
	/// [`router::Router::unhandled_notification`]) are installed.
	#[error("{0}")]
	Routing(String),
	/// Failed to spawn the language server process.
	#[error("failed to spawn LSP server '{server}': {reason}")]
	ServerSpawn {
		/// The server command that failed.
		server: String,
		/// The failure reason.
		reason: String,
	},
	/// Outbound queue is full. Retry later.
	#[error("outbound queue full (backpressure)")]
	Backpressure,
	/// Server not yet initialized. Retry after initialization completes.
	#[error("server not ready")]
	NotReady,
}

impl From<serde_json::Error> for Error {
	fn from(e: serde_json::Error) -> Self {
		Self::Deserialize(e.to_string())
	}
}

impl From<io::Error> for Error {
	fn from(e: io::Error) -> Self {
		Self::Io(e.to_string())
	}
}

/// Converts a filesystem path to an LSP URI.
///
/// Relative paths are canonicalized to absolute paths first.
/// Paths are percent-encoded as required by the LSP URI format.
/// Returns `None` if the path cannot be converted.
pub fn uri_from_path(path: &std::path::Path) -> Option<lsp_types::Uri> {
	use std::str::FromStr;

	let abs_path = if path.is_absolute() {
		path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
	} else {
		path.canonicalize().or_else(|_| std::env::current_dir().map(|cwd| cwd.join(path))).ok()?
	};

	let url = url::Url::from_file_path(abs_path).ok()?;
	lsp_types::Uri::from_str(url.as_str()).ok()
}

/// Converts an LSP URI to a filesystem path.
///
/// Returns `None` if the URI is not a `file://` scheme or cannot be parsed.
pub fn path_from_uri(uri: &lsp_types::Uri) -> Option<std::path::PathBuf> {
	use std::str::FromStr;

	let url = url::Url::from_str(uri.as_str()).ok()?;
	url.to_file_path().ok()
}

/// The core service abstraction, representing either a Language Server or Language Client.
pub trait LspService: Service<AnyRequest> {
	/// The handler of [LSP notifications](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#notificationMessage).
	///
	/// Notifications are delivered in order and synchronously. This is mandatory since they can
	/// change the interpretation of later notifications or requests.
	///
	/// # Return
	///
	/// The return value decides the action to either break or continue the main loop.
	fn notify(&mut self, notif: AnyNotification) -> ControlFlow<Result<()>>;

	/// The handler of an arbitrary [`AnyEvent`].
	///
	/// Events are emitted by users or middlewares via [`ClientSocket::emit`] or
	/// [`ServerSocket::emit`], for user-defined purposes. Events are delivered in order and
	/// synchronously.
	///
	/// # Return
	///
	/// The return value decides the action to either break or continue the main loop.
	fn emit(&mut self, event: AnyEvent) -> ControlFlow<Result<()>>;
}
