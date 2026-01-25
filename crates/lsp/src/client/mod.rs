//! LSP client wrapper for spawning and communicating with language servers.
//!
//! This module provides the [`ClientHandle`] type which wraps an LSP language server
//! process and provides methods for sending requests and notifications.
//!
//! # Architecture
//!
//! The client spawns a language server process and communicates via stdin/stdout.
//! It uses the [`crate::MainLoop`] to drive the LSP protocol, running in a
//! background task. The client provides a [`ServerSocket`] for sending requests
//! and notifications.
//!
//! # Event Handling
//!
//! Server-to-client notifications (diagnostics, progress, etc.) are delivered via
//! the [`LspEventHandler`] trait. Implement this trait to receive LSP events:
//!
//! ```ignore
//! use xeno_lsp::client::{LspEventHandler, LanguageServerId};
//!
//! struct MyHandler;
//!
//! impl LspEventHandler for MyHandler {
//!     fn on_diagnostics(
//!         &self,
//!         _server_id: LanguageServerId,
//!         uri: Uri,
//!         diagnostics: Vec<Diagnostic>,
//!         _version: Option<i32>,
//!     ) {
//!         // Update UI with new diagnostics
//!     }
//! }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use xeno_lsp::client::{Client, ServerConfig, LanguageServerId};
//!
//! let config = ServerConfig::new("rust-analyzer", "/path/to/project");
//! let client = Client::start(LanguageServerId(1), "rust-analyzer".into(), config)?;
//!
//! // Initialize the server
//! client.initialize(true).await?;
//!
//! // Use the client for LSP operations
//! let hover = client.hover(uri, position).await?;
//! ```

// Existing files (unchanged)
mod capabilities;
mod config;
mod event_handler;

// New split modules
mod api;
mod handle;
mod lifecycle;
mod outbox;
mod router_setup;
mod state;

// Public re-exports (preserve existing API surface)
pub use capabilities::client_capabilities;
pub use config::{LanguageServerId, OffsetEncoding, ServerConfig};
pub use event_handler::{LogLevel, LspEventHandler, NoOpEventHandler, SharedEventHandler};
pub use handle::ClientHandle;
pub use lifecycle::start_server;
pub use state::ServerState;
