//! Asynchronous [Language Server Protocol (LSP)][lsp] framework based on [tower].
//!
//! [lsp]: https://microsoft.github.io/language-server-protocol/overviews/lsp/overview/
//! [tower]: https://github.com/tower-rs/tower
//!
//! This crate is centered at a core service trait [`LspService`] for either Language Servers or
//! Language Clients. The main loop driver [`MainLoop`] executes the service. The additional
//! features, called middleware, are pluggable can be layered using the [`tower_layer`]
//! abstraction. This crate defines several common middlewares for various mandatory or optional
//! LSP functionalities, see their documentations for details.
//! - [`concurrency::Concurrency`]: Incoming request multiplexing and cancellation.
//! - [`panic::CatchUnwind`]: Turn panics into errors.
//! - [`tracing::Tracing`]: Logger spans with methods instrumenting handlers.
//! - [`server::Lifecycle`]: Server initialization, shutting down, and exit handling.
//! - [`client_monitor::ClientProcessMonitor`]: Client process monitor.
//! - [`router::Router`]: "Root" service to dispatch requests, notifications and events.
//!
//! Users are free to select and layer middlewares to run a Language Server or Language Client.
//! They can also implement their own middlewares for like timeout, metering, request
//! transformation and etc.
//!
//! ## Usages
//!
//! There are two main ways to define a [`Router`](router::Router) root service: one is via its
//! builder API, and the other is to construct via implementing the omnitrait [`LanguageServer`] or
//! [`LanguageClient`] for a state struct. The former is more flexible, while the latter has a
//! more similar API as [`tower-lsp`](https://crates.io/crates/tower-lsp).
//!
//! ## Cargo features
//!
//! - `client-monitor`: Client process monitor middleware [`client_monitor`].
//!   *Enabled by default.*
//! - `omni-trait`: Mega traits of all standard requests and notifications, namely
//!   [`LanguageServer`] and [`LanguageClient`].
//!   *Enabled by default.*
//! - `stdio`: Utilities to deal with pipe-like stdin/stdout communication channel for Language
//!   Servers.
//!   *Enabled by default.*
//! - `forward`: Impl [`LspService`] for `{Client,Server}Socket`. This collides some method names
//!   but allows easy service forwarding.
//!   *Disabled by default.*
//! - `tokio`: Enable compatible methods for [`tokio`](https://crates.io/crates/tokio) runtime.
//!   *Disabled by default.*
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
mod socket;
mod types;

pub use event::AnyEvent;
pub use mainloop::MainLoop;
pub use socket::{ClientSocket, ServerSocket};
pub use types::{AnyNotification, AnyRequest, ErrorCode, RequestId, ResponseError};

pub mod concurrency;
pub mod panic;
pub mod router;
pub mod server;

/// Service forwarding implementations (requires `forward` feature).
#[cfg(feature = "forward")]
#[cfg_attr(docsrs, doc(cfg(feature = "forward")))]
mod forward;

#[cfg(feature = "client-monitor")]
#[cfg_attr(docsrs, doc(cfg(feature = "client-monitor")))]
pub mod client_monitor;

#[cfg(all(feature = "stdio", unix))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "stdio", unix))))]
pub mod stdio;

pub mod tracing;

/// Mega-traits for Language Server and Client implementations.
#[cfg(feature = "omni-trait")]
mod omni_trait;
#[cfg(feature = "omni-trait")]
#[cfg_attr(docsrs, doc(cfg(feature = "omni-trait")))]
pub use omni_trait::{LanguageClient, LanguageServer};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
#[cfg(feature = "client")]
pub use client::{
	ClientHandle, LanguageServerId, LogLevel, LspEventHandler, NoOpEventHandler, OffsetEncoding,
	ServerConfig, SharedEventHandler, start_server,
};

#[cfg(feature = "position")]
#[cfg_attr(docsrs, doc(cfg(feature = "position")))]
pub mod position;
#[cfg(feature = "position")]
pub use position::{
	char_range_to_lsp_range, char_to_lsp_position, lsp_position_to_char, lsp_range_to_char_range,
};

#[cfg(feature = "position")]
#[cfg_attr(docsrs, doc(cfg(feature = "position")))]
/// LSP change computation helpers.
pub mod changes;
#[cfg(feature = "position")]
pub use changes::compute_lsp_changes;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod registry;
#[cfg(feature = "client")]
pub use registry::{LanguageServerConfig, Registry};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod document;
#[cfg(feature = "client")]
pub use document::{
	DiagnosticsEvent, DiagnosticsEventReceiver, DiagnosticsEventSender, DocumentState,
	DocumentStateManager,
};

#[cfg(feature = "position")]
#[cfg_attr(docsrs, doc(cfg(feature = "position")))]
pub mod sync;
#[cfg(feature = "position")]
pub use sync::{DocumentSync, DocumentSyncEventHandler};

/// A convenient type alias for `Result` with `E` = [`enum@crate::Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Possible errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
	/// The service main loop stopped.
	#[error("service stopped")]
	ServiceStopped,
	/// The peer replies undecodable or invalid responses.
	#[error("deserialization failed: {0}")]
	Deserialize(#[from] serde_json::Error),
	/// The peer replies an error.
	#[error("{0}")]
	Response(#[from] ResponseError),
	/// The peer violates the Language Server Protocol.
	#[error("protocol error: {0}")]
	Protocol(String),
	/// Input/output errors from the underlying channels.
	#[error("{0}")]
	Io(#[from] io::Error),
	/// The underlying channel reached EOF (end of file).
	#[error("the underlying channel reached EOF")]
	Eof,
	/// No handlers for events or mandatory notifications (not starting with `$/`).
	///
	/// Will not occur when catch-all handlers ([`router::Router::unhandled_event`] and
	/// [`router::Router::unhandled_notification`]) are installed.
	#[error("{0}")]
	Routing(String),
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
