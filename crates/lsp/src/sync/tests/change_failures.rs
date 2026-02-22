use super::*;

#[tokio::test]
async fn did_change_failure_marks_force_full_sync() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(InitRecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/change_fail.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();
	assert!(documents.is_opened(&uri));

	// Wait for initialization to complete.
	let client = registry.get("rust", path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "client must be initialized");

	// No force_full_sync initially.
	assert!(!documents.take_force_full_sync_by_uri(&uri));

	// Make didChange fail.
	transport.set_fail_method("textDocument/didChange");

	let result = sync
		.send_change(ChangeRequest::full_text(path, "rust", "fn main() { 1 }".into()).with_open_if_needed(false))
		.await;
	assert!(result.is_err(), "expected error from failed didChange");

	// force_full_sync must be set after failure.
	assert!(
		documents.take_force_full_sync_by_uri(&uri),
		"force_full_sync must be set after didChange failure"
	);
}

#[tokio::test]
async fn did_change_success_does_not_set_force_full_sync() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(InitRecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/change_ok.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();

	// Wait for initialization.
	let client = registry.get("rust", path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "client must be initialized");

	let result = sync
		.send_change(ChangeRequest::full_text(path, "rust", "fn main() { 1 }".into()).with_open_if_needed(false))
		.await;
	assert!(result.is_ok(), "expected successful didChange");

	// force_full_sync must NOT be set after success.
	assert!(
		!documents.take_force_full_sync_by_uri(&uri),
		"force_full_sync should not be set after successful didChange"
	);
}

#[tokio::test]
async fn open_document_unregisters_if_did_open_fails() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/rope_fail_open.rs");
	let uri = crate::uri_from_path(path).unwrap();

	// Make didOpen fail.
	transport.set_fail_method("textDocument/didOpen");

	let result = sync.open_document(path, "rust", &Rope::from("fn main() {}")).await;
	assert!(result.is_err(), "expected error from failed didOpen via open_document");

	// URI must NOT be registered or opened.
	assert!(!documents.is_opened(&uri), "phantom open via Rope API");
	assert!(documents.get_diagnostics(&uri).is_empty());
}

#[tokio::test]
async fn open_document_can_retry_after_failed_open() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/rope_retry_open.rs");
	let uri = crate::uri_from_path(path).unwrap();

	// First attempt fails.
	transport.set_fail_method("textDocument/didOpen");
	let result = sync.open_document(path, "rust", &Rope::from("fn main() {}")).await;
	assert!(result.is_err());
	assert!(!documents.is_opened(&uri), "state must be clean after failure");

	// Clear failure and retry.
	transport.clear_fail_method("textDocument/didOpen");
	let result = sync.open_document(path, "rust", &Rope::from("fn main() {}")).await;
	assert!(result.is_ok(), "retry must succeed: {:?}", result.err());
	assert!(documents.is_opened(&uri), "document must be opened after retry");
}
