//! LSP client stack: transport, session management, and document synchronization.
//!
//! This crate provides the client-side LSP integration layer built on top of
//! the [`xeno_lsp_framework`] tower-based LSP framework.
//!
//! ## Cargo features
//!
//! * `client`: Client transport, process management, server config types.
//! * `position`: Position conversion and change computation utilities (implies `client`).
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

// Re-export framework types that downstream (editor) uses via `xeno_lsp::*`.
pub use xeno_lsp_framework::{
	AnyEvent, AnyNotification, AnyRequest, AnyResponse, Error, ErrorCode, JsonValue, Message, OffsetEncoding, RequestId, ResponseError, Result, lsp_types,
	path_from_uri, uri_from_path,
};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
#[cfg(feature = "client")]
pub use client::{ClientHandle, LanguageServerId, LocalTransport, LogLevel, LspEventHandler, NoOpEventHandler, ServerConfig, ServerState, SharedEventHandler};
#[cfg(feature = "position")]
pub use xeno_lsp_framework::{
	IncrementalResult, char_range_to_lsp_range, char_to_lsp_position, compute_lsp_changes, lsp_position_to_char, lsp_range_to_char_range,
};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod registry;
#[cfg(feature = "client")]
pub use registry::{AcquireDisposition, AcquireResult, LanguageServerConfig, Registry};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod document;
#[cfg(feature = "client")]
pub use document::{DiagnosticsEvent, DiagnosticsEventReceiver, DiagnosticsEventSender, DocumentState, DocumentStateManager};

#[cfg(feature = "position")]
#[cfg_attr(docsrs, doc(cfg(feature = "position")))]
pub mod sync;
#[cfg(feature = "position")]
pub use sync::{BarrierMode, ChangeDispatch, ChangePayload, ChangeRequest, DocumentSync, DocumentSyncEventHandler};

#[cfg(all(feature = "client", feature = "position"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "client", feature = "position"))))]
/// LSP session management (completion, etc.).
pub mod session;

#[cfg(all(feature = "client", feature = "position"))]
pub use session::{CompletionController, CompletionRequest, CompletionTrigger, LspRuntime, LspSession, RuntimeStartError};
