use std::path::Path;
use std::sync::Arc;

use ropey::Rope;

use super::*;

/// Verifies that pull diagnostics request is sent and diagnostics are returned.
#[tokio::test]
async fn pull_diagnostics_returns_full_report() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		diagnostic_provider: Some(lsp_types::DiagnosticServerCapabilities::Options(lsp_types::DiagnosticOptions {
			identifier: None,
			inter_file_dependencies: false,
			workspace_diagnostics: false,
			work_done_progress_options: Default::default(),
		})),
		..Default::default()
	};

	let transport = Arc::new(InitRecordingTransport::with_capabilities(caps));

	let full_report =
		lsp_types::DocumentDiagnosticReportResult::Report(lsp_types::DocumentDiagnosticReport::Full(lsp_types::RelatedFullDocumentDiagnosticReport {
			related_documents: None,
			full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
				result_id: Some("result-1".into()),
				items: vec![lsp_types::Diagnostic {
					range: lsp_types::Range {
						start: lsp_types::Position { line: 0, character: 4 },
						end: lsp_types::Position { line: 0, character: 5 },
					},
					severity: Some(lsp_types::DiagnosticSeverity::ERROR),
					message: "unused variable".into(),
					..Default::default()
				}],
			},
		}));
	transport
		.inner
		.set_request_response("textDocument/diagnostic", serde_json::to_value(&full_report).unwrap());

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
	assert!(client.supports_pull_diagnostics());

	transport.inner.messages.lock().unwrap().clear();

	let uri = crate::uri_from_path(file).unwrap();
	let result = client.pull_diagnostics(uri, None).await.unwrap();
	assert!(result.is_some());

	let methods = transport.inner.recorded_methods();
	assert!(
		methods.contains(&"textDocument/diagnostic".to_string()),
		"textDocument/diagnostic request sent; methods: {methods:?}"
	);
}

/// Verifies that the diagnostic refresh signal is emitted and cleared correctly.
#[tokio::test]
async fn diagnostic_refresh_signals_editor() {
	use crate::registry::LanguageServerConfig;

	let caps = lsp_types::ServerCapabilities {
		diagnostic_provider: Some(lsp_types::DiagnosticServerCapabilities::Options(lsp_types::DiagnosticOptions {
			identifier: None,
			inter_file_dependencies: false,
			workspace_diagnostics: false,
			work_done_progress_options: Default::default(),
		})),
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
	assert!(!sync.take_diagnostic_refresh());

	// Signal the refresh.
	sync.signal_diagnostic_refresh();

	// The flag should be set.
	assert!(sync.take_diagnostic_refresh());

	// After taking, it should be cleared.
	assert!(!sync.take_diagnostic_refresh());
}

/// Verifies that servers without pull diagnostic support return None.
#[tokio::test]
async fn pull_diagnostics_not_supported_returns_none() {
	use crate::registry::LanguageServerConfig;

	// Server with no diagnostic_provider.
	let caps = lsp_types::ServerCapabilities::default();
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
	let client = registry.get("rust", file).unwrap();
	for _ in 0..100 {
		if client.is_initialized() {
			break;
		}
		tokio::task::yield_now().await;
	}

	assert!(!client.supports_pull_diagnostics());

	let uri = crate::uri_from_path(file).unwrap();
	let result = client.pull_diagnostics(uri, None).await.unwrap();
	assert!(result.is_none());
}
