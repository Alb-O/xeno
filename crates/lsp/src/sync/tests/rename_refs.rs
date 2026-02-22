use std::path::Path;
use std::sync::Arc;

use lsp_types::Uri;
use ropey::Rope;

use super::*;

/// Verifies that references request is sent and locations are returned.
#[tokio::test]
async fn references_returns_locations() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		references_provider: Some(lsp_types::OneOf::Left(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let locations = vec![
		lsp_types::Location {
			uri: "file:///project/src/main.rs".parse::<Uri>().unwrap(),
			range: lsp_types::Range {
				start: lsp_types::Position { line: 0, character: 3 },
				end: lsp_types::Position { line: 0, character: 7 },
			},
		},
		lsp_types::Location {
			uri: "file:///project/src/lib.rs".parse::<Uri>().unwrap(),
			range: lsp_types::Range {
				start: lsp_types::Position { line: 5, character: 0 },
				end: lsp_types::Position { line: 5, character: 4 },
			},
		},
	];
	transport
		.inner
		.set_request_response("textDocument/references", serde_json::to_value(&locations).unwrap());

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let file = Path::new("/project/src/main.rs");
	sync.open_document(file, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", file).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());

	transport.inner.messages.lock().unwrap().clear();

	let uri: Uri = crate::uri_from_path(file).unwrap();
	let position = lsp_types::Position { line: 0, character: 3 };
	let result = client.references(uri, position, true).await.unwrap();
	assert!(result.is_some());
	let locs = result.unwrap();
	assert_eq!(locs.len(), 2);

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/references".to_string()),
		"references request sent; methods: {methods:?}"
	);
}

/// Verifies that prepare_rename returns a placeholder.
#[tokio::test]
async fn prepare_rename_returns_placeholder() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		rename_provider: Some(lsp_types::OneOf::Right(lsp_types::RenameOptions {
			prepare_provider: Some(true),
			work_done_progress_options: Default::default(),
		})),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let response = lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 0, character: 3 },
			end: lsp_types::Position { line: 0, character: 7 },
		},
		placeholder: "main".into(),
	};
	transport
		.inner
		.set_request_response("textDocument/prepareRename", serde_json::to_value(&response).unwrap());

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let file = Path::new("/project/src/main.rs");
	sync.open_document(file, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", file).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());
	assert!(client.supports_prepare_rename());

	transport.inner.messages.lock().unwrap().clear();

	let uri: Uri = crate::uri_from_path(file).unwrap();
	let position = lsp_types::Position { line: 0, character: 3 };
	let result = client.prepare_rename(uri, position).await.unwrap();
	assert!(result.is_some());
	match result.unwrap() {
		lsp_types::PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
			assert_eq!(placeholder, "main");
		}
		other => panic!("expected RangeWithPlaceholder, got {other:?}"),
	}

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/prepareRename".to_string()),
		"prepareRename request sent; methods: {methods:?}"
	);
}

/// Verifies that rename returns a workspace edit.
#[tokio::test]
async fn rename_returns_workspace_edit() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		rename_provider: Some(lsp_types::OneOf::Left(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let mut changes = std::collections::HashMap::new();
	changes.insert(
		"file:///project/src/main.rs".parse::<Uri>().unwrap(),
		vec![lsp_types::TextEdit {
			range: lsp_types::Range {
				start: lsp_types::Position { line: 0, character: 3 },
				end: lsp_types::Position { line: 0, character: 7 },
			},
			new_text: "start".into(),
		}],
	);
	let edit = lsp_types::WorkspaceEdit {
		changes: Some(changes),
		..Default::default()
	};
	transport
		.inner
		.set_request_response("textDocument/rename", serde_json::to_value(&edit).unwrap());

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let file = Path::new("/project/src/main.rs");
	sync.open_document(file, "rust", &Rope::from("fn main() {}")).await.unwrap();
	let client = registry.get("rust", file).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());

	transport.inner.messages.lock().unwrap().clear();

	let uri: Uri = crate::uri_from_path(file).unwrap();
	let position = lsp_types::Position { line: 0, character: 3 };
	let result = client.rename(uri, position, "start".into()).await.unwrap();
	assert!(result.is_some());
	let ws_edit = result.unwrap();
	assert!(ws_edit.changes.is_some());
	let changes = ws_edit.changes.unwrap();
	assert_eq!(changes.len(), 1);

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/rename".to_string()),
		"rename request sent; methods: {methods:?}"
	);
}
