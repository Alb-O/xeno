use std::path::Path;
use std::sync::Arc;

use lsp_types::Uri;
use ropey::Rope;

use super::*;

/// Verifies that formatting sends the request and returns text edits.
#[tokio::test]
async fn formatting_returns_edits() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let edits = vec![lsp_types::TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 0, character: 0 },
			end: lsp_types::Position { line: 0, character: 5 },
		},
		new_text: "formatted".into(),
	}];
	transport
		.inner
		.set_request_response("textDocument/formatting", serde_json::to_value(&edits).unwrap());

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
	let options = lsp_types::FormattingOptions {
		tab_size: 4,
		insert_spaces: false,
		..Default::default()
	};
	let result = client.formatting(uri, options).await.unwrap();
	assert!(result.is_some());
	let result_edits = result.unwrap();
	assert_eq!(result_edits.len(), 1);
	assert_eq!(result_edits[0].new_text, "formatted");

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/formatting".to_string()),
		"formatting request sent; methods: {methods:?}"
	);
}

/// Verifies that range_formatting sends the request and returns text edits.
#[tokio::test]
async fn range_formatting_returns_edits() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		document_range_formatting_provider: Some(lsp_types::OneOf::Left(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let edits = vec![lsp_types::TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 2, character: 0 },
			end: lsp_types::Position { line: 2, character: 8 },
		},
		new_text: "    let x".into(),
	}];
	transport
		.inner
		.set_request_response("textDocument/rangeFormatting", serde_json::to_value(&edits).unwrap());

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let file = Path::new("/project/src/main.rs");
	sync.open_document(file, "rust", &Rope::from("fn main() {\nlet x = 1;\n}")).await.unwrap();
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
	let range = lsp_types::Range {
		start: lsp_types::Position { line: 1, character: 0 },
		end: lsp_types::Position { line: 1, character: 10 },
	};
	let options = lsp_types::FormattingOptions {
		tab_size: 4,
		insert_spaces: true,
		..Default::default()
	};
	let result = client.range_formatting(uri, range, options).await.unwrap();
	assert!(result.is_some());
	let result_edits = result.unwrap();
	assert_eq!(result_edits.len(), 1);
	assert_eq!(result_edits[0].new_text, "    let x");

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/rangeFormatting".to_string()),
		"rangeFormatting request sent; methods: {methods:?}"
	);
}
