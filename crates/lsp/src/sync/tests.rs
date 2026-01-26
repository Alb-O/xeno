use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

use super::*;

#[test]
fn test_document_sync_with_registry() {
	let registry = Arc::new(Registry::new());
	let documents = Arc::new(DocumentStateManager::new());
	let sync = DocumentSync::with_registry(registry, documents);

	assert_eq!(sync.total_error_count(), 0);
	assert_eq!(sync.total_warning_count(), 0);
}

#[test]
fn test_document_sync_create() {
	let (sync, _registry, _documents, _receiver) = DocumentSync::create();

	assert_eq!(sync.total_error_count(), 0);
	assert_eq!(sync.total_warning_count(), 0);
}

#[test]
fn test_diagnostics_event_updates_state() {
	let documents = Arc::new(DocumentStateManager::new());
	let handler = DocumentSyncEventHandler::new(documents.clone());
	let uri: Uri = "file:///test.rs".parse().expect("valid uri");

	handler.on_diagnostics(
		LanguageServerId(1),
		uri.clone(),
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "test error".to_string(),
			..Diagnostic::default()
		}],
		None,
	);

	let diagnostics = documents.get_diagnostics(&uri);
	assert_eq!(diagnostics.len(), 1);
}
