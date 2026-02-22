use std::path::Path;
use std::sync::Arc;

use lsp_types::Uri;
use ropey::Rope;

use super::*;

/// Verifies that goto_declaration sends the request and returns a Location response.
#[tokio::test]
async fn goto_declaration_returns_location() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		declaration_provider: Some(lsp_types::DeclarationCapability::Simple(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let loc = lsp_types::Location {
		uri: "file:///project/src/lib.rs".parse::<Uri>().unwrap(),
		range: lsp_types::Range {
			start: lsp_types::Position { line: 10, character: 4 },
			end: lsp_types::Position { line: 10, character: 12 },
		},
	};
	let response = lsp_types::GotoDefinitionResponse::Scalar(loc.clone());
	transport
		.inner
		.set_request_response("textDocument/declaration", serde_json::to_value(response).unwrap());

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
	let result = client.goto_declaration(uri, position).await.unwrap();
	assert!(result.is_some());
	match result.unwrap() {
		lsp_types::GotoDefinitionResponse::Scalar(l) => {
			assert_eq!(l.range.start.line, 10);
			assert_eq!(l.range.start.character, 4);
		}
		other => panic!("expected Scalar, got {other:?}"),
	}

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/declaration".to_string()),
		"declaration request sent; methods: {methods:?}"
	);
}

/// Verifies that goto_implementation sends the request and returns a LocationLink response.
#[tokio::test]
async fn goto_implementation_returns_location_link() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		implementation_provider: Some(lsp_types::ImplementationProviderCapability::Simple(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let link = lsp_types::LocationLink {
		origin_selection_range: None,
		target_uri: "file:///project/src/impl.rs".parse::<Uri>().unwrap(),
		target_range: lsp_types::Range {
			start: lsp_types::Position { line: 5, character: 0 },
			end: lsp_types::Position { line: 20, character: 1 },
		},
		target_selection_range: lsp_types::Range {
			start: lsp_types::Position { line: 5, character: 4 },
			end: lsp_types::Position { line: 5, character: 15 },
		},
	};
	let response = lsp_types::GotoDefinitionResponse::Link(vec![link]);
	transport
		.inner
		.set_request_response("textDocument/implementation", serde_json::to_value(response).unwrap());

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
	let result = client.goto_implementation(uri, position).await.unwrap();
	assert!(result.is_some());
	match result.unwrap() {
		lsp_types::GotoDefinitionResponse::Link(links) => {
			assert_eq!(links.len(), 1);
			assert_eq!(links[0].target_selection_range.start.line, 5);
			assert_eq!(links[0].target_selection_range.start.character, 4);
		}
		other => panic!("expected Link, got {other:?}"),
	}

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/implementation".to_string()),
		"implementation request sent; methods: {methods:?}"
	);
}

/// Verifies that goto_type_definition sends the request and returns a response.
#[tokio::test]
async fn goto_type_definition_returns_response() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		type_definition_provider: Some(lsp_types::TypeDefinitionProviderCapability::Simple(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let loc = lsp_types::Location {
		uri: "file:///project/src/types.rs".parse::<Uri>().unwrap(),
		range: lsp_types::Range {
			start: lsp_types::Position { line: 3, character: 7 },
			end: lsp_types::Position { line: 3, character: 15 },
		},
	};
	let response = lsp_types::GotoDefinitionResponse::Array(vec![loc]);
	transport
		.inner
		.set_request_response("textDocument/typeDefinition", serde_json::to_value(response).unwrap());

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
	let result = client.goto_type_definition(uri, position).await.unwrap();
	assert!(result.is_some());
	match result.unwrap() {
		lsp_types::GotoDefinitionResponse::Array(locs) => {
			assert_eq!(locs.len(), 1);
			assert_eq!(locs[0].range.start.line, 3);
		}
		other => panic!("expected Array, got {other:?}"),
	}

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/typeDefinition".to_string()),
		"typeDefinition request sent; methods: {methods:?}"
	);
}
