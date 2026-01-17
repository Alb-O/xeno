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
mod tests {
	use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

	use super::*;

	fn make_diagnostic(severity: DiagnosticSeverity, message: &str) -> Diagnostic {
		Diagnostic {
			range: Range::default(),
			severity: Some(severity),
			code: None,
			code_description: None,
			source: Some("test".into()),
			message: message.into(),
			related_information: None,
			tags: None,
			data: None,
		}
	}

	#[test]
	fn test_document_state_version() {
		let uri = "file:///test.rs".parse().unwrap();
		let state = DocumentState::from_uri(uri);

		assert_eq!(state.version(), 0);
		assert_eq!(state.increment_version(), 1);
		assert_eq!(state.increment_version(), 2);
		assert_eq!(state.version(), 2);
	}

	#[test]
	fn test_document_state_diagnostics() {
		let uri = "file:///test.rs".parse().unwrap();
		let state = DocumentState::from_uri(uri);

		assert!(!state.has_errors());
		assert!(!state.has_warnings());

		let diagnostics = vec![
			make_diagnostic(DiagnosticSeverity::ERROR, "error 1"),
			make_diagnostic(DiagnosticSeverity::ERROR, "error 2"),
			make_diagnostic(DiagnosticSeverity::WARNING, "warning 1"),
		];
		state.set_diagnostics(diagnostics);

		assert!(state.has_errors());
		assert!(state.has_warnings());
		assert_eq!(state.error_count(), 2);
		assert_eq!(state.warning_count(), 1);
	}

	#[test]
	fn test_document_state_manager() {
		let manager = DocumentStateManager::new();
		let uri = "file:///test.rs".parse().unwrap();

		let path = PathBuf::from("/test.rs");
		manager.register(&path, Some("rust"));
		assert!(manager.contains(&uri));

		let diagnostics = vec![make_diagnostic(DiagnosticSeverity::ERROR, "test error")];
		manager.update_diagnostics(&uri, diagnostics, None);
		assert_eq!(manager.get_diagnostics(&uri).len(), 1);
		assert_eq!(manager.total_error_count(), 1);

		manager.unregister(&uri);
		assert!(!manager.contains(&uri));
	}

	#[test]
	fn test_document_state_manager_versions_monotonic() {
		let manager = DocumentStateManager::new();
		let path = PathBuf::from("/test.rs");
		let uri = manager.register(&path, Some("rust")).unwrap();

		let v1 = manager.queue_change(&uri).unwrap();
		let v2 = manager.queue_change(&uri).unwrap();

		assert!(v2 > v1);
	}

	#[test]
	fn test_document_state_manager_mismatch_forces_full_sync() {
		let manager = DocumentStateManager::new();
		let path = PathBuf::from("/test.rs");
		let uri = manager.register(&path, Some("rust")).unwrap();

		let version = manager.queue_change(&uri).unwrap();
		assert!(manager.ack_change(&uri, version));

		manager.update_diagnostics(&uri, Vec::new(), Some(version.saturating_sub(1)));

		assert!(manager.take_force_full_sync_by_uri(&uri));
		assert_eq!(manager.pending_change_count(&uri), 0);
	}
}
