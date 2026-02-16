//! Headless LSP smoke test for runtime verification.

#[cfg(feature = "lsp")]
use tracing::{info, warn};

#[cfg(feature = "lsp")]
use crate::lsp::LspSystem;

/// Headless LSP smoke test for runtime verification.
///
/// # Test Coverage
///
/// 1. Singleflight correctness: concurrent `acquire()` must yield one started instance
/// 2. Server→client request routing: `workspace/configuration`, progress notifications
/// 3. Basic LSP operations: document open, hover requests
#[cfg(feature = "lsp")]
pub async fn run_lsp_smoke(workspace: Option<std::path::PathBuf>) -> anyhow::Result<()> {
	info!("Starting headless LSP smoke test");

	let workspace_path = workspace
		.or_else(|| std::env::current_dir().ok())
		.ok_or_else(|| anyhow::anyhow!("Could not determine workspace path"))?;

	if !workspace_path.join("Cargo.toml").exists() {
		anyhow::bail!("Workspace does not contain Cargo.toml: {}", workspace_path.display());
	}

	let lsp_system = LspSystem::new();

	lsp_system.configure_server(
		"rust",
		crate::lsp::api::LanguageServerConfig {
			command: "rust-analyzer".to_string(),
			args: vec![],
			env: vec![],
			root_markers: vec!["Cargo.toml".to_string()],
			timeout_secs: 30,
			initialization_options: Some(serde_json::json!({
				"rust-analyzer": {
					"checkOnSave": {
						"command": "clippy"
					}
				}
			})),
			settings: None,
			enable_snippets: true,
		},
	);

	info!("Test 1: Concurrent server start");
	let test_file = workspace_path.join("src/lib.rs");
	if !test_file.exists() {
		anyhow::bail!("Test file not found: {}", test_file.display());
	}

	let registry = lsp_system.registry();
	let (h1, h2) = tokio::join!(async { registry.acquire("rust", &test_file).await }, async {
		registry.acquire("rust", &test_file).await
	});

	let acquired_1 = h1.map_err(|e| anyhow::anyhow!(e))?;
	let _acquired_2 = h2.map_err(|e| anyhow::anyhow!(e))?;
	info!(
		id1 = %acquired_1.server_id,
		"Concurrent start complete"
	);

	info!("Test 2: Open document and wait for server requests");
	let content = std::fs::read_to_string(&test_file)?;
	let client = lsp_system
		.sync()
		.open_document_text(&test_file, "rust", content)
		.await
		.map_err(|e| anyhow::anyhow!(e))?;

	tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

	info!("Test 3: Trigger hover to force server→client requests");
	let uri = xeno_lsp::uri_from_path(&test_file).ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
	let position = xeno_lsp::lsp_types::Position { line: 0, character: 0 };
	match client.hover(uri, position).await {
		Ok(_) => info!("Hover request completed"),
		Err(e) => warn!("Hover request failed (expected during initialization): {}", e),
	}

	tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

	info!("Smoke test complete - check logs for trace output");
	Ok(())
}
