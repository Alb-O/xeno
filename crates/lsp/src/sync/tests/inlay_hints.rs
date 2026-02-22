use std::path::Path;
use std::sync::Arc;

use lsp_types::Uri;
use ropey::Rope;

use super::*;

/// Verifies that inlay hint request is sent and hints are returned.
#[tokio::test]
async fn inlay_hints_returns_hints() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		inlay_hint_provider: Some(lsp_types::OneOf::Left(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let hints = vec![lsp_types::InlayHint {
		position: lsp_types::Position { line: 0, character: 8 },
		label: lsp_types::InlayHintLabel::String(": i32".into()),
		kind: Some(lsp_types::InlayHintKind::TYPE),
		text_edits: None,
		tooltip: None,
		padding_left: Some(true),
		padding_right: None,
		data: None,
	}];
	transport
		.inner
		.set_request_response("textDocument/inlayHint", serde_json::to_value(&hints).unwrap());

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let file = Path::new("/project/src/main.rs");
	sync.open_document(file, "rust", &Rope::from("let x = 42;")).await.unwrap();
	let client = registry.get("rust", file).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());
	assert!(client.supports_inlay_hint());

	transport.inner.messages.lock().unwrap().clear();

	let uri: Uri = crate::uri_from_path(file).unwrap();
	let range = lsp_types::Range {
		start: lsp_types::Position { line: 0, character: 0 },
		end: lsp_types::Position { line: 0, character: 11 },
	};
	let result = client.inlay_hints(uri, range).await.unwrap();
	assert!(result.is_some());
	let result_hints = result.unwrap();
	assert_eq!(result_hints.len(), 1);
	match &result_hints[0].label {
		lsp_types::InlayHintLabel::String(s) => assert_eq!(s, ": i32"),
		other => panic!("expected String label, got {other:?}"),
	}

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/inlayHint".to_string()),
		"inlayHint request sent; methods: {methods:?}"
	);
}

/// Verifies that inlay hint resolve is sent when server supports it.
#[tokio::test]
async fn inlay_hint_resolve_fetches_details() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		inlay_hint_provider: Some(lsp_types::OneOf::Right(lsp_types::InlayHintServerCapabilities::Options(
			lsp_types::InlayHintOptions {
				resolve_provider: Some(true),
				work_done_progress_options: Default::default(),
			},
		))),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let resolved = lsp_types::InlayHint {
		position: lsp_types::Position { line: 0, character: 8 },
		label: lsp_types::InlayHintLabel::String(": i32".into()),
		kind: Some(lsp_types::InlayHintKind::TYPE),
		text_edits: None,
		tooltip: Some(lsp_types::InlayHintTooltip::String("inferred type".into())),
		padding_left: Some(true),
		padding_right: None,
		data: Some(serde_json::json!({"id": 1})),
	};
	transport
		.inner
		.set_request_response("inlayHint/resolve", serde_json::to_value(&resolved).unwrap());

	let (sync, registry, _documents, _receiver) = DocumentSync::create(transport.clone());

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let file = Path::new("/project/src/main.rs");
	sync.open_document(file, "rust", &Rope::from("let x = 42;")).await.unwrap();
	let client = registry.get("rust", file).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}
	assert!(client.is_initialized());
	assert!(client.supports_inlay_hint_resolve());

	transport.inner.messages.lock().unwrap().clear();

	let hint = lsp_types::InlayHint {
		position: lsp_types::Position { line: 0, character: 8 },
		label: lsp_types::InlayHintLabel::String(": i32".into()),
		kind: Some(lsp_types::InlayHintKind::TYPE),
		text_edits: None,
		tooltip: None,
		padding_left: Some(true),
		padding_right: None,
		data: Some(serde_json::json!({"id": 1})),
	};
	let result = client.inlay_hint_resolve(hint).await.unwrap();
	assert!(result.tooltip.is_some());

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"inlayHint/resolve".to_string()),
		"inlayHint/resolve request sent; methods: {methods:?}"
	);
}

/// Verifies that the inlay hint refresh signal is emitted when the server sends the request.
#[tokio::test]
async fn inlay_hint_refresh_signals_editor() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		inlay_hint_provider: Some(lsp_types::OneOf::Left(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

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

	// Initially no refresh signaled.
	assert!(!sync.take_inlay_hint_refresh());

	// Signal the refresh.
	sync.signal_inlay_hint_refresh();

	// The flag should be set.
	assert!(sync.take_inlay_hint_refresh());

	// After taking, it should be cleared.
	assert!(!sync.take_inlay_hint_refresh());
}
