//! LSP event handler trait and types.
//!
//! This module defines the [`LspEventHandler`] trait for receiving
//! server-to-client notifications from language servers.

use std::sync::Arc;

use lsp_types::{Diagnostic, ProgressParams, Uri};

use super::config::LanguageServerId;

/// Handler for LSP server-to-client events.
///
/// Implement this trait to receive notifications from language servers.
/// All methods have default no-op implementations, so you only need to
/// implement the events you care about.
pub trait LspEventHandler: Send + Sync {
	/// Called when the server publishes diagnostics for a document.
	fn on_diagnostics(
		&self,
		_server_id: LanguageServerId,
		_uri: Uri,
		_diagnostics: Vec<Diagnostic>,
		_version: Option<i32>,
	) {
	}

	/// Called when the server reports progress (e.g., "Indexing...").
	fn on_progress(&self, _server_id: LanguageServerId, _params: ProgressParams) {}

	/// Called when the server sends a log message.
	fn on_log_message(&self, _server_id: LanguageServerId, _level: LogLevel, _message: &str) {}

	/// Called when the server wants to show a message to the user.
	fn on_show_message(&self, _server_id: LanguageServerId, _level: LogLevel, _message: &str) {}
}

/// Log level for server messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
	/// Error message.
	Error,
	/// Warning message.
	Warning,
	/// Informational message.
	Info,
	/// Debug/log message.
	Debug,
}

impl From<lsp_types::MessageType> for LogLevel {
	fn from(typ: lsp_types::MessageType) -> Self {
		match typ {
			lsp_types::MessageType::ERROR => LogLevel::Error,
			lsp_types::MessageType::WARNING => LogLevel::Warning,
			lsp_types::MessageType::INFO => LogLevel::Info,
			_ => LogLevel::Debug,
		}
	}
}

/// A no-op event handler that ignores all events.
///
/// Use this when you don't need to handle server events.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpEventHandler;

impl LspEventHandler for NoOpEventHandler {}

/// Type alias for a shared event handler.
pub type SharedEventHandler = Arc<dyn LspEventHandler>;
