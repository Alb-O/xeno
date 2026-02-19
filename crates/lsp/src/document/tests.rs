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

#[test]
fn test_generation_starts_at_zero_before_mark_opened() {
	let manager = DocumentStateManager::new();
	let path = PathBuf::from("/test.rs");
	let uri = manager.register(&path, Some("rust")).unwrap();

	assert_eq!(manager.doc_generation(&uri), Some(0), "generation should be 0 before mark_opened");
}

#[test]
fn test_mark_opened_assigns_unique_generations() {
	let manager = DocumentStateManager::new();
	let path_a = PathBuf::from("/a.rs");
	let path_b = PathBuf::from("/b.rs");
	let uri_a = manager.register(&path_a, Some("rust")).unwrap();
	let uri_b = manager.register(&path_b, Some("rust")).unwrap();

	manager.mark_opened(&uri_a, 0);
	let gen_a = manager.doc_generation(&uri_a).unwrap();

	manager.mark_opened(&uri_b, 0);
	let gen_b = manager.doc_generation(&uri_b).unwrap();

	assert_ne!(gen_a, gen_b, "each mark_opened should produce a unique generation");
	assert!(gen_b > gen_a);
}

#[test]
fn test_generation_none_after_unregister() {
	let manager = DocumentStateManager::new();
	let path = PathBuf::from("/test.rs");
	let uri = manager.register(&path, Some("rust")).unwrap();
	manager.mark_opened(&uri, 0);
	assert!(manager.doc_generation(&uri).is_some());

	manager.unregister(&uri);
	assert_eq!(manager.doc_generation(&uri), None, "generation should be None after unregister");
}

#[test]
fn test_generation_changes_on_reopen() {
	let manager = DocumentStateManager::new();
	let path = PathBuf::from("/test.rs");

	// First open session.
	let uri = manager.register(&path, Some("rust")).unwrap();
	manager.mark_opened(&uri, 0);
	let gen_first = manager.doc_generation(&uri).unwrap();

	// Close (unregister) and reopen.
	manager.unregister(&uri);
	let uri = manager.register(&path, Some("rust")).unwrap();
	manager.mark_opened(&uri, 0);
	let gen_second = manager.doc_generation(&uri).unwrap();

	assert_ne!(gen_first, gen_second, "reopened doc must get a different generation");
}
