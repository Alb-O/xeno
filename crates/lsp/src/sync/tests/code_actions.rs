use std::path::Path;
use std::sync::Arc;

use lsp_types::Uri;
use ropey::Rope;

use super::*;

/// Verifies that code_action request is sent and inline edit is returned.
#[tokio::test]
async fn code_action_inline_edit_returned() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	// Canned response: one code action with an inline edit.
	let edit = lsp_types::WorkspaceEdit {
		changes: Some(std::collections::HashMap::new()),
		..Default::default()
	};
	let action = lsp_types::CodeAction {
		title: "Fix import".into(),
		edit: Some(edit),
		..Default::default()
	};
	let response = vec![lsp_types::CodeActionOrCommand::CodeAction(action)];
	transport
		.inner
		.set_request_response("textDocument/codeAction", serde_json::to_value(response).unwrap());

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
	let range = lsp_types::Range::default();
	let context = lsp_types::CodeActionContext {
		diagnostics: vec![],
		only: None,
		trigger_kind: None,
	};

	let result = client.code_action(uri, range, context).await.unwrap();
	assert!(result.is_some());
	let actions = result.unwrap();
	assert_eq!(actions.len(), 1);
	match &actions[0] {
		lsp_types::CodeActionOrCommand::CodeAction(a) => {
			assert_eq!(a.title, "Fix import");
			assert!(a.edit.is_some());
		}
		_ => panic!("expected CodeAction"),
	}

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/codeAction".to_string()),
		"codeAction request sent; methods: {methods:?}"
	);
}

/// Verifies that code_action_resolve sends the resolve request and returns the resolved action.
#[tokio::test]
async fn code_action_resolve_fetches_edit() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(lsp_types::CodeActionOptions {
			resolve_provider: Some(true),
			..Default::default()
		})),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	// Canned resolve response: action now has an edit.
	let resolved = lsp_types::CodeAction {
		title: "Organize imports".into(),
		edit: Some(lsp_types::WorkspaceEdit {
			changes: Some(std::collections::HashMap::new()),
			..Default::default()
		}),
		data: Some(serde_json::json!({"id": 42})),
		..Default::default()
	};
	transport
		.inner
		.set_request_response("codeAction/resolve", serde_json::to_value(&resolved).unwrap());

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
	assert!(client.supports_code_action_resolve());

	transport.inner.messages.lock().unwrap().clear();

	// Unresolved action: has data but no edit/command.
	let unresolved = lsp_types::CodeAction {
		title: "Organize imports".into(),
		data: Some(serde_json::json!({"id": 42})),
		..Default::default()
	};

	let result = client.code_action_resolve(unresolved).await.unwrap();
	assert_eq!(result.title, "Organize imports");
	assert!(result.edit.is_some(), "resolved action should have edit");

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"codeAction/resolve".to_string()),
		"resolve request sent; methods: {methods:?}"
	);
}

/// Verifies that execute_command sends the workspace/executeCommand request.
#[tokio::test]
async fn execute_command_sends_request() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		execute_command_provider: Some(lsp_types::ExecuteCommandOptions {
			commands: vec!["my.command".into()],
			..Default::default()
		}),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));
	transport.inner.set_request_response("workspace/executeCommand", serde_json::Value::Null);

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

	let result = client.execute_command("my.command".into(), Some(vec![serde_json::json!({"key": "val"})])).await;
	assert!(result.is_ok());

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"workspace/executeCommand".to_string()),
		"executeCommand request sent; methods: {methods:?}"
	);
}
