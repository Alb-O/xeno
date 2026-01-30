//! Common utilities for editor integration tests.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use xeno_broker::core::BrokerCore;
use xeno_broker::ipc;
use xeno_broker::test_helpers::TestLauncher;
use xeno_lsp::client::ServerConfig;

/// Type alias for the tuple returned by spawn_broker.
pub type SpawnedBroker = (
	std::path::PathBuf,
	Arc<BrokerCore>,
	TestLauncher,
	CancellationToken,
	tempfile::TempDir,
);

/// Spawns a broker instance for testing with a unique socket path.
pub async fn spawn_broker() -> SpawnedBroker {
	let _ = tracing_subscriber::fmt::try_init();
	let tmp = tempfile::tempdir().expect("failed to create temp dir");
	let sock = tmp.path().join("broker.sock");
	let core = BrokerCore::new();
	let launcher = TestLauncher::new();
	let shutdown = CancellationToken::new();

	let core_clone = core.clone();
	let launcher_clone = Arc::new(launcher.clone());
	let sock_clone = sock.clone();
	let shutdown_clone = shutdown.clone();

	tokio::spawn(async move {
		if let Err(e) =
			ipc::serve_with_launcher(sock_clone, core_clone, shutdown_clone, launcher_clone).await
		{
			tracing::error!(error = %e, "Broker serve failed");
		}
	});

	// Wait for socket to be ready (attempt connect instead of just exists)
	let mut attempts = 0;
	while attempts < 100 {
		if let Ok(_stream) = tokio::net::UnixStream::connect(&sock).await {
			break;
		}
		tokio::time::sleep(Duration::from_millis(10)).await;
		attempts += 1;
	}

	(sock, core, launcher, shutdown, tmp)
}

/// Creates a standard server configuration for testing.
#[must_use]
pub fn test_server_config() -> ServerConfig {
	ServerConfig::new("rust-analyzer", "/test")
}

/// Polls a condition with a timeout.
pub async fn wait_until<F, Fut>(timeout: Duration, mut f: F) -> bool
where
	F: FnMut() -> Fut,
	Fut: std::future::Future<Output = bool>,
{
	let start = std::time::Instant::now();
	while start.elapsed() < timeout {
		if f().await {
			return true;
		}
		tokio::time::sleep(Duration::from_millis(10)).await;
	}
	false
}
