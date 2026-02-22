use super::*;

/// Simulates the rename-file flow: willRenameFiles → didClose → didOpen → didRenameFiles.
///
/// Verifies that all four messages are sent in the correct order when a file
/// is renamed through the LSP client API and DocumentSync::reopen_document.
#[tokio::test]
async fn rename_file_sequence_will_close_open_did() {
	use crate::registry::LanguageServerConfig;

	let file_op_filter = lsp_types::FileOperationRegistrationOptions {
		filters: vec![lsp_types::FileOperationFilter {
			scheme: None,
			pattern: lsp_types::FileOperationPattern {
				glob: "**/*.rs".into(),
				matches: None,
				options: None,
			},
		}],
	};
	let caps = lsp_types::ServerCapabilities {
		workspace: Some(lsp_types::WorkspaceServerCapabilities {
			file_operations: Some(lsp_types::WorkspaceFileOperationsServerCapabilities {
				will_rename: Some(file_op_filter.clone()),
				did_rename: Some(file_op_filter),
				..Default::default()
			}),
			..Default::default()
		}),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	// Set up a null response for willRenameFiles.
	transport.inner.set_request_response("workspace/willRenameFiles", serde_json::Value::Null);

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/project/src/old.rs");
	let new_path = Path::new("/project/src/new.rs");

	// Open the document and wait for initialization.
	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", old_path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "client must be initialized");

	// Clear recordings from setup.
	transport.inner.messages.lock().unwrap().clear();

	// 1. willRenameFiles request.
	let old_uri = crate::uri_from_path(old_path).unwrap();
	let new_uri = crate::uri_from_path(new_path).unwrap();
	let _edit = client
		.will_rename_files(vec![lsp_types::FileRename {
			old_uri: old_uri.to_string(),
			new_uri: new_uri.to_string(),
		}])
		.await
		.unwrap();

	// 2. reopen_document: didClose(old) + didOpen(new).
	sync.reopen_document(old_path, "rust", new_path, "rust", "fn main() {}".into()).await.unwrap();

	// 3. didRenameFiles notification.
	client
		.did_rename_files(vec![lsp_types::FileRename {
			old_uri: old_uri.to_string(),
			new_uri: new_uri.to_string(),
		}])
		.await
		.unwrap();

	// Verify ordering: willRename → didClose → didOpen → didRename.
	let methods = transport.inner.recorded_methods();
	let will_idx = methods.iter().position(|m| m == "workspace/willRenameFiles");
	let close_idx = methods.iter().position(|m| m == "textDocument/didClose");
	let open_idx = methods.iter().position(|m| m == "textDocument/didOpen");
	let did_idx = methods.iter().position(|m| m == "workspace/didRenameFiles");

	assert!(will_idx.is_some(), "willRenameFiles not sent; methods: {methods:?}");
	assert!(close_idx.is_some(), "didClose not sent; methods: {methods:?}");
	assert!(open_idx.is_some(), "didOpen not sent; methods: {methods:?}");
	assert!(did_idx.is_some(), "didRenameFiles not sent; methods: {methods:?}");

	let will_idx = will_idx.unwrap();
	let close_idx = close_idx.unwrap();
	let open_idx = open_idx.unwrap();
	let did_idx = did_idx.unwrap();

	assert!(will_idx < close_idx, "willRename must precede didClose; methods: {methods:?}");
	assert!(close_idx < open_idx, "didClose must precede didOpen; methods: {methods:?}");
	assert!(open_idx < did_idx, "didOpen must precede didRename; methods: {methods:?}");
}

/// When the server doesn't advertise fileOperations rename support, will/did
/// rename messages must not be emitted.
#[tokio::test]
async fn rename_file_no_capability_skips_will_and_did() {
	use crate::registry::LanguageServerConfig;

	let transport = Arc::new(InitRecordingTransport::new());
	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let old_path = Path::new("/project/src/nocap.rs");
	let new_path = Path::new("/project/src/nocap_new.rs");

	sync.open_document(old_path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", old_path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized(), "client must be initialized");

	transport.inner.messages.lock().unwrap().clear();

	// will_rename_files should return None when not supported.
	let old_uri = crate::uri_from_path(old_path).unwrap();
	let new_uri = crate::uri_from_path(new_path).unwrap();
	let edit = client
		.will_rename_files(vec![lsp_types::FileRename {
			old_uri: old_uri.to_string(),
			new_uri: new_uri.to_string(),
		}])
		.await
		.unwrap();
	assert!(edit.is_none(), "willRenameFiles should return None when unsupported");

	// did_rename_files should be a noop.
	client
		.did_rename_files(vec![lsp_types::FileRename {
			old_uri: old_uri.to_string(),
			new_uri: new_uri.to_string(),
		}])
		.await
		.unwrap();

	let methods = transport.inner.recorded_methods();
	assert!(
		!methods.iter().any(|m| m.contains("Rename")),
		"no rename messages should be sent without capability; methods: {methods:?}"
	);
}

/// Verifies the create-file sequence: willCreateFiles → didOpen → didCreateFiles.
#[tokio::test]
async fn create_file_sequence_will_open_did() {
	use crate::registry::LanguageServerConfig;

	let file_op_filter = lsp_types::FileOperationRegistrationOptions {
		filters: vec![lsp_types::FileOperationFilter {
			scheme: None,
			pattern: lsp_types::FileOperationPattern {
				glob: "**/*.rs".into(),
				matches: None,
				options: None,
			},
		}],
	};
	let caps = lsp_types::ServerCapabilities {
		workspace: Some(lsp_types::WorkspaceServerCapabilities {
			file_operations: Some(lsp_types::WorkspaceFileOperationsServerCapabilities {
				will_create: Some(file_op_filter.clone()),
				did_create: Some(file_op_filter),
				..Default::default()
			}),
			..Default::default()
		}),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));
	transport.inner.set_request_response("workspace/willCreateFiles", serde_json::Value::Null);

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	// Open an existing file to acquire a client.
	let existing = Path::new("/project/src/existing.rs");
	sync.open_document(existing, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", existing).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());

	transport.inner.messages.lock().unwrap().clear();

	let new_path = Path::new("/project/src/new_file.rs");
	let new_uri = crate::uri_from_path(new_path).unwrap();

	// 1. willCreateFiles
	let _edit = client
		.will_create_files(vec![lsp_types::FileCreate { uri: new_uri.to_string() }])
		.await
		.unwrap();

	// 2. didOpen (via open_document)
	sync.open_document(new_path, "rust", &Rope::from("")).await.unwrap();

	// 3. didCreateFiles
	client.did_create_files(vec![lsp_types::FileCreate { uri: new_uri.to_string() }]).await.unwrap();

	let methods = transport.inner.recorded_methods();
	let will_idx = methods.iter().position(|m| m == "workspace/willCreateFiles");
	let open_idx = methods.iter().position(|m| m == "textDocument/didOpen");
	let did_idx = methods.iter().position(|m| m == "workspace/didCreateFiles");

	assert!(will_idx.is_some(), "willCreateFiles not sent; methods: {methods:?}");
	assert!(open_idx.is_some(), "didOpen not sent; methods: {methods:?}");
	assert!(did_idx.is_some(), "didCreateFiles not sent; methods: {methods:?}");

	assert!(will_idx.unwrap() < open_idx.unwrap(), "willCreate must precede didOpen; methods: {methods:?}");
	assert!(open_idx.unwrap() < did_idx.unwrap(), "didOpen must precede didCreate; methods: {methods:?}");
}

/// Verifies the delete-file sequence: willDeleteFiles → didClose → didDeleteFiles.
#[tokio::test]
async fn delete_file_sequence_will_close_did() {
	use crate::registry::LanguageServerConfig;

	let file_op_filter = lsp_types::FileOperationRegistrationOptions {
		filters: vec![lsp_types::FileOperationFilter {
			scheme: None,
			pattern: lsp_types::FileOperationPattern {
				glob: "**/*.rs".into(),
				matches: None,
				options: None,
			},
		}],
	};
	let caps = lsp_types::ServerCapabilities {
		workspace: Some(lsp_types::WorkspaceServerCapabilities {
			file_operations: Some(lsp_types::WorkspaceFileOperationsServerCapabilities {
				will_delete: Some(file_op_filter.clone()),
				did_delete: Some(file_op_filter),
				..Default::default()
			}),
			..Default::default()
		}),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));
	transport.inner.set_request_response("workspace/willDeleteFiles", serde_json::Value::Null);

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("/project/src/delete_me.rs");
	sync.open_document(path, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", path).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());

	transport.inner.messages.lock().unwrap().clear();

	let uri = crate::uri_from_path(path).unwrap();

	// 1. willDeleteFiles
	let _edit = client.will_delete_files(vec![lsp_types::FileDelete { uri: uri.to_string() }]).await.unwrap();

	// 2. didClose
	sync.close_document(path, "rust").await.unwrap();

	// 3. didDeleteFiles
	client.did_delete_files(vec![lsp_types::FileDelete { uri: uri.to_string() }]).await.unwrap();

	let methods = transport.inner.recorded_methods();
	let will_idx = methods.iter().position(|m| m == "workspace/willDeleteFiles");
	let close_idx = methods.iter().position(|m| m == "textDocument/didClose");
	let did_idx = methods.iter().position(|m| m == "workspace/didDeleteFiles");

	assert!(will_idx.is_some(), "willDeleteFiles not sent; methods: {methods:?}");
	assert!(close_idx.is_some(), "didClose not sent; methods: {methods:?}");
	assert!(did_idx.is_some(), "didDeleteFiles not sent; methods: {methods:?}");

	assert!(will_idx.unwrap() < close_idx.unwrap(), "willDelete must precede didClose; methods: {methods:?}");
	assert!(close_idx.unwrap() < did_idx.unwrap(), "didClose must precede didDelete; methods: {methods:?}");
}

/// Verifies the mkdir sequence: willCreateFiles → didCreateFiles (no didOpen).
#[tokio::test]
async fn mkdir_sequence_will_did_create() {
	use crate::registry::LanguageServerConfig;

	let file_op_filter = lsp_types::FileOperationRegistrationOptions {
		filters: vec![lsp_types::FileOperationFilter {
			scheme: None,
			pattern: lsp_types::FileOperationPattern {
				glob: "**/*".into(),
				matches: None,
				options: None,
			},
		}],
	};
	let caps = lsp_types::ServerCapabilities {
		workspace: Some(lsp_types::WorkspaceServerCapabilities {
			file_operations: Some(lsp_types::WorkspaceFileOperationsServerCapabilities {
				will_create: Some(file_op_filter.clone()),
				did_create: Some(file_op_filter),
				..Default::default()
			}),
			..Default::default()
		}),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));
	transport.inner.set_request_response("workspace/willCreateFiles", serde_json::Value::Null);

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	// Open a file to acquire a client.
	let existing = Path::new("/project/src/lib.rs");
	sync.open_document(existing, "rust", &Rope::from("")).await.unwrap();
	let client = registry.get("rust", existing).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());

	transport.inner.messages.lock().unwrap().clear();

	let dir_uri = "file:///project/src/new_dir";

	// 1. willCreateFiles
	let _edit = client.will_create_files(vec![lsp_types::FileCreate { uri: dir_uri.into() }]).await.unwrap();

	// 2. didCreateFiles (no didOpen for directories)
	client.did_create_files(vec![lsp_types::FileCreate { uri: dir_uri.into() }]).await.unwrap();

	let methods = transport.inner.recorded_methods();
	let will_idx = methods.iter().position(|m| m == "workspace/willCreateFiles");
	let did_idx = methods.iter().position(|m| m == "workspace/didCreateFiles");
	let open_idx = methods.iter().position(|m| m == "textDocument/didOpen");

	assert!(will_idx.is_some(), "willCreateFiles not sent; methods: {methods:?}");
	assert!(did_idx.is_some(), "didCreateFiles not sent; methods: {methods:?}");
	assert!(open_idx.is_none(), "didOpen should NOT be sent for directories; methods: {methods:?}");
	assert!(will_idx.unwrap() < did_idx.unwrap(), "willCreate must precede didCreate; methods: {methods:?}");
}

/// Verifies the rmdir sequence: willDeleteFiles → didDeleteFiles (no didClose).
#[tokio::test]
async fn rmdir_sequence_will_did_delete() {
	use crate::registry::LanguageServerConfig;

	let file_op_filter = lsp_types::FileOperationRegistrationOptions {
		filters: vec![lsp_types::FileOperationFilter {
			scheme: None,
			pattern: lsp_types::FileOperationPattern {
				glob: "**/*".into(),
				matches: None,
				options: None,
			},
		}],
	};
	let caps = lsp_types::ServerCapabilities {
		workspace: Some(lsp_types::WorkspaceServerCapabilities {
			file_operations: Some(lsp_types::WorkspaceFileOperationsServerCapabilities {
				will_delete: Some(file_op_filter.clone()),
				did_delete: Some(file_op_filter),
				..Default::default()
			}),
			..Default::default()
		}),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));
	transport.inner.set_request_response("workspace/willDeleteFiles", serde_json::Value::Null);

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let existing = Path::new("/project/src/lib.rs");
	sync.open_document(existing, "rust", &Rope::from("")).await.unwrap();
	let client = registry.get("rust", existing).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());

	transport.inner.messages.lock().unwrap().clear();

	let dir_uri = "file:///project/src/old_dir";

	// 1. willDeleteFiles
	let _edit = client.will_delete_files(vec![lsp_types::FileDelete { uri: dir_uri.into() }]).await.unwrap();

	// 2. didDeleteFiles (no didClose for directories)
	client.did_delete_files(vec![lsp_types::FileDelete { uri: dir_uri.into() }]).await.unwrap();

	let methods = transport.inner.recorded_methods();
	let will_idx = methods.iter().position(|m| m == "workspace/willDeleteFiles");
	let did_idx = methods.iter().position(|m| m == "workspace/didDeleteFiles");
	let close_idx = methods.iter().position(|m| m == "textDocument/didClose");

	assert!(will_idx.is_some(), "willDeleteFiles not sent; methods: {methods:?}");
	assert!(did_idx.is_some(), "didDeleteFiles not sent; methods: {methods:?}");
	assert!(close_idx.is_none(), "didClose should NOT be sent for directories; methods: {methods:?}");
	assert!(will_idx.unwrap() < did_idx.unwrap(), "willDelete must precede didDelete; methods: {methods:?}");
}
