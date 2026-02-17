use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::*;
use crate::client::transport::StartedServer;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, ResponseError};

struct MockTransport {
	start_count: AtomicUsize,
	started_notify: Arc<tokio::sync::Notify>,
	finish_notify: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl LspTransport for MockTransport {
	fn subscribe_events(&self) -> crate::Result<tokio::sync::mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		let (_, rx) = tokio::sync::mpsc::unbounded_channel();
		Ok(rx)
	}

	async fn start(&self, cfg: ServerConfig) -> Result<StartedServer> {
		self.start_count.fetch_add(1, Ordering::SeqCst);
		self.started_notify.notify_one();
		self.finish_notify.notified().await;
		Ok(StartedServer { id: cfg.id })
	}

	async fn notify(&self, _server: LanguageServerId, _notif: AnyNotification) -> Result<()> {
		Ok(())
	}

	async fn notify_with_barrier(&self, _server: LanguageServerId, _notif: AnyNotification) -> Result<tokio::sync::oneshot::Receiver<crate::Result<()>>> {
		let (tx, rx) = tokio::sync::oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}

	async fn request(&self, _server: LanguageServerId, _req: AnyRequest, _timeout: Option<Duration>) -> Result<AnyResponse> {
		unimplemented!()
	}

	async fn reply(&self, _server: LanguageServerId, _id: crate::types::RequestId, _resp: std::result::Result<Value, ResponseError>) -> Result<()> {
		Ok(())
	}

	async fn stop(&self, _server: LanguageServerId) -> Result<()> {
		Ok(())
	}
}

#[tokio::test]
async fn test_acquire_singleflight() {
	let started_notify = Arc::new(tokio::sync::Notify::new());
	let finish_notify = Arc::new(tokio::sync::Notify::new());
	let transport = Arc::new(MockTransport {
		start_count: AtomicUsize::new(0),
		started_notify: started_notify.clone(),
		finish_notify: finish_notify.clone(),
	});
	let registry = Arc::new(Registry::new(transport.clone(), xeno_worker::WorkerRuntime::new()));

	registry.register(
		"rust",
		LanguageServerConfig {
			command: "rust-analyzer".into(),
			..Default::default()
		},
	);

	let path = Path::new("test.rs");

	let r1 = registry.clone();
	let r2 = registry.clone();

	let h1_fut = tokio::spawn(async move { r1.acquire("rust", path).await });

	// Wait for leader to enter transport.start()
	started_notify.notified().await;

	// Join concurrent caller
	let h2_fut = tokio::spawn(async move { r2.acquire("rust", path).await });

	// Give h2 a moment to surely be waiting on the watch channel
	tokio::time::sleep(Duration::from_millis(50)).await;

	// Let leader finish
	finish_notify.notify_one();

	let (h1, h2) = tokio::join!(h1_fut, h2_fut);

	let h1 = h1.unwrap();
	let h2 = h2.unwrap();

	assert!(h1.is_ok());
	assert!(h2.is_ok());
	assert_eq!(transport.start_count.load(Ordering::SeqCst), 1);
	let h1 = h1.unwrap();
	let h2 = h2.unwrap();
	assert_eq!(h1.server_id, h2.server_id);
	assert_eq!(h1.disposition, AcquireDisposition::Started);
	assert_eq!(h2.disposition, AcquireDisposition::Started);
}
