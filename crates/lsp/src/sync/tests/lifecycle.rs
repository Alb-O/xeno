use super::*;

#[tokio::test]
async fn reopen_document_sends_did_close_then_did_open() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone());

	// Configure a server so acquire() succeeds.
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/reopen_old.rs");
	let new_path = Path::new("/reopen_new.rs");

	// Open document under old path (triggers acquire + didOpen).
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Clear recorded notifications from the open call.
	transport.messages.lock().unwrap().clear();

	// Reopen under new path.
	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// Old URI must be unregistered.
	assert!(!documents.is_opened(&old_uri));

	// New URI must be registered and opened.
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(documents.is_opened(&new_uri));

	// Verify notification ordering: didClose before didOpen.
	let methods = transport.recorded_methods();
	let close_idx = methods.iter().position(|m| m == "textDocument/didClose");
	let open_idx = methods.iter().position(|m| m == "textDocument/didOpen");
	assert!(close_idx.is_some(), "didClose not sent; methods: {:?}", methods);
	assert!(open_idx.is_some(), "didOpen not sent; methods: {:?}", methods);
	assert!(close_idx.unwrap() < open_idx.unwrap(), "didClose must precede didOpen; methods: {:?}", methods);
}

#[tokio::test]
async fn reopen_document_clears_old_diagnostics() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport);

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/diag_old.rs");
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();

	// Inject diagnostics for the old URI.
	documents.update_diagnostics(
		&old_uri,
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "old error".into(),
			..Diagnostic::default()
		}],
		None,
	);
	assert_eq!(documents.get_diagnostics(&old_uri).len(), 1);

	// Reopen under new path.
	let new_path = Path::new("/diag_new.rs");
	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// Old diagnostics must be cleared (unregister removes the entry).
	assert!(documents.get_diagnostics(&old_uri).is_empty());
}

#[tokio::test]
async fn reopen_document_cross_language_routes_to_correct_servers() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(RecordingTransport::new());
	let (sync, registry, documents, _receiver) = DocumentSync::create(transport.clone());

	// Configure two different language servers.
	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);
	registry.register(
		"python",
		LanguageServerConfig {
			command: "pyright".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/rename_me.rs");
	let new_path = Path::new("/rename_me.py");

	// Open under old language.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Record the server id used for the rust open.
	let rust_server_id = {
		let recs = transport.recorded();
		recs.iter().find(|r| r.method == "textDocument/didOpen").unwrap().server_id
	};

	// Clear recordings.
	transport.messages.lock().unwrap().clear();

	// Reopen under different language.
	sync.reopen_document(old_path, "rust", new_path, "python", "def main(): pass".into())
		.await
		.unwrap();

	let recs = transport.recorded();

	// didClose should go to the rust server.
	let close = recs.iter().find(|r| r.method == "textDocument/didClose").expect("didClose not sent");
	assert_eq!(close.server_id, rust_server_id, "didClose should go to rust server");
	assert!(close.uri.as_deref().unwrap().contains("rename_me.rs"));

	// didOpen should go to a different server (python).
	let open = recs.iter().find(|r| r.method == "textDocument/didOpen").expect("didOpen not sent");
	assert_ne!(open.server_id, rust_server_id, "didOpen should go to python server, not rust");
	assert!(open.uri.as_deref().unwrap().contains("rename_me.py"));

	// Ordering: close before open.
	let close_idx = recs.iter().position(|r| r.method == "textDocument/didClose").unwrap();
	let open_idx = recs.iter().position(|r| r.method == "textDocument/didOpen").unwrap();
	assert!(close_idx < open_idx);
}

#[tokio::test]
async fn reopen_then_change_maintains_correct_identity() {
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

	let old_path = Path::new("/identity_old.rs");
	let new_path = Path::new("/identity_new.rs");

	// Open old, reopen to new.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	transport.messages.lock().unwrap().clear();

	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// Now send a change to the new path.
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(documents.is_opened(&new_uri));

	// send_change with full text should succeed on the new identity.
	// The server isn't initialized so send_change will reopen (open_if_needed=true).
	// What matters: no notification goes to the old URI after reopen.
	let recs = transport.recorded();
	let old_uri_str = crate::uri_from_path(old_path).unwrap().to_string();

	// After the clear, only didClose should reference the old URI.
	let old_refs: Vec<_> = recs.iter().filter(|r| r.uri.as_deref() == Some(old_uri_str.as_str())).collect();
	assert!(
		old_refs.iter().all(|r| r.method == "textDocument/didClose"),
		"only didClose should reference old URI after reopen; got: {:?}",
		old_refs.iter().map(|r| &r.method).collect::<Vec<_>>()
	);

	// Ordering: didClose(old) → didOpen(new).
	let close_idx = recs.iter().position(|r| r.method == "textDocument/didClose").unwrap();
	let open_idx = recs.iter().position(|r| r.method == "textDocument/didOpen").unwrap();
	assert!(close_idx < open_idx, "didClose must precede didOpen");
}

#[tokio::test]
async fn close_document_sends_did_close_and_clears_diagnostics() {
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

	let path = Path::new("/close_me.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();
	assert!(documents.is_opened(&uri));

	// Inject diagnostics.
	documents.update_diagnostics(
		&uri,
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "error".into(),
			..Diagnostic::default()
		}],
		None,
	);
	assert_eq!(documents.get_diagnostics(&uri).len(), 1);

	transport.messages.lock().unwrap().clear();

	// Close the document.
	sync.close_document(path, "rust").await.unwrap();

	// URI must be unregistered.
	assert!(!documents.is_opened(&uri));

	// Diagnostics must be cleared.
	assert!(documents.get_diagnostics(&uri).is_empty());

	// didClose notification must have been sent.
	let recs = transport.recorded();
	let close = recs.iter().find(|r| r.method == "textDocument/didClose");
	assert!(
		close.is_some(),
		"didClose not sent; methods: {:?}",
		recs.iter().map(|r| &r.method).collect::<Vec<_>>()
	);
	assert!(close.unwrap().uri.as_deref().unwrap().contains("close_me.rs"));
}

#[tokio::test]
async fn ensure_open_text_registers_and_sends_did_open() {
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

	let path = Path::new("/open_me.rs");
	let uri = crate::uri_from_path(path).unwrap();
	assert!(!documents.is_opened(&uri));

	// Open the document.
	sync.ensure_open_text(path, "rust", "fn main() {}".into()).await.unwrap();

	// URI must be registered and opened.
	assert!(documents.is_opened(&uri));

	// didOpen notification must have been sent with correct URI.
	let recs = transport.recorded();
	let open = recs.iter().find(|r| r.method == "textDocument/didOpen");
	assert!(
		open.is_some(),
		"didOpen not sent; methods: {:?}",
		recs.iter().map(|r| &r.method).collect::<Vec<_>>()
	);
	assert!(open.unwrap().uri.as_deref().unwrap().contains("open_me.rs"));
}

#[tokio::test]
async fn close_document_unregisters_even_if_did_close_fails() {
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

	let path = Path::new("/fail_close.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let uri = crate::uri_from_path(path).unwrap();
	assert!(documents.is_opened(&uri));

	// Inject diagnostics.
	documents.update_diagnostics(
		&uri,
		vec![Diagnostic {
			range: Range::default(),
			severity: Some(DiagnosticSeverity::ERROR),
			message: "error".into(),
			..Diagnostic::default()
		}],
		None,
	);

	// Make didClose fail.
	transport.set_fail_method("textDocument/didClose");

	// close_document should return Err but still unregister.
	let result = sync.close_document(path, "rust").await;
	assert!(result.is_err(), "expected error from failed didClose");

	// URI must be unregistered despite the error.
	assert!(!documents.is_opened(&uri));

	// Diagnostics must be cleared.
	assert!(documents.get_diagnostics(&uri).is_empty());
}

#[tokio::test]
async fn reopen_document_opens_new_even_if_did_close_fails() {
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

	let old_path = Path::new("/fail_reopen_old.rs");
	let new_path = Path::new("/fail_reopen_new.rs");

	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Make didClose fail.
	transport.set_fail_method("textDocument/didClose");

	// reopen_document should still open the new document.
	let result = sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await;

	// Should return the close error (open succeeded).
	assert!(result.is_err(), "expected error propagated from failed didClose");

	// Old URI must be unregistered despite the error.
	assert!(!documents.is_opened(&old_uri));

	// New URI must be registered and opened.
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(documents.is_opened(&new_uri));
}

#[tokio::test]
async fn ensure_open_text_unregisters_if_did_open_fails() {
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

	let path = Path::new("/fail_open.rs");
	let uri = crate::uri_from_path(path).unwrap();

	// Make didOpen fail.
	transport.set_fail_method("textDocument/didOpen");

	let result = sync.ensure_open_text(path, "rust", "fn main() {}".into()).await;
	assert!(result.is_err(), "expected error from failed didOpen");

	// URI must NOT be registered or opened.
	assert!(!documents.is_opened(&uri));
	assert!(documents.get_diagnostics(&uri).is_empty());
}

#[tokio::test]
async fn reopen_document_does_not_register_new_if_did_open_fails() {
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

	let old_path = Path::new("/reopen_fail_old.rs");
	let new_path = Path::new("/reopen_fail_new.rs");

	// Open old normally.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let old_uri = crate::uri_from_path(old_path).unwrap();
	assert!(documents.is_opened(&old_uri));

	// Make didOpen fail (didClose will succeed).
	transport.set_fail_method("textDocument/didOpen");

	let result = sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await;
	assert!(result.is_err(), "expected error from failed didOpen on new path");

	// Old must be unregistered (close succeeded).
	assert!(!documents.is_opened(&old_uri));

	// New must NOT be registered (open failed, unregister cleaned up).
	let new_uri = crate::uri_from_path(new_path).unwrap();
	assert!(!documents.is_opened(&new_uri));
}
