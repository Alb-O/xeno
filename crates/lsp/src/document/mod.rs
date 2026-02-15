//! LSP document state tracking.
//!
//! This module provides types for tracking LSP-related state for documents,
//! including version numbers, diagnostics, and language server associations.

mod manager;
mod progress;
mod state;

use std::path::PathBuf;

pub use manager::DocumentStateManager;
pub use progress::ProgressItem;
pub use state::DocumentState;
use tokio::sync::mpsc;

/// Event emitted when diagnostics are updated for a document.
#[derive(Debug, Clone)]
pub struct DiagnosticsEvent {
	/// Path to the document (derived from URI).
	pub path: PathBuf,
	/// Number of error diagnostics.
	pub error_count: usize,
	/// Number of warning diagnostics.
	pub warning_count: usize,
}

/// Sender for diagnostic events.
pub type DiagnosticsEventSender = mpsc::UnboundedSender<DiagnosticsEvent>;

/// Receiver for diagnostic events.
pub type DiagnosticsEventReceiver = mpsc::UnboundedReceiver<DiagnosticsEvent>;

#[cfg(test)]
mod tests;
