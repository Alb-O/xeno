use std::time::Duration;

use tokio::time::{sleep, timeout};
use xeno_worker::ShutdownMode;

use super::FsService;
use crate::filesystem::FilesystemOptions;

async fn wait_until<F>(name: &str, mut condition: F)
where
	F: FnMut() -> bool,
{
	timeout(Duration::from_secs(2), async move {
		loop {
			if condition() {
				return;
			}
			sleep(Duration::from_millis(10)).await;
		}
	})
	.await
	.unwrap_or_else(|_| panic!("timed out waiting for {name}"));
}

#[tokio::test]
async fn ensure_index_with_same_spec_does_not_restart() {
	let mut service = FsService::new();
	let root = tempfile::tempdir().expect("must create tempdir");

	assert!(service.ensure_index(root.path().to_path_buf(), FilesystemOptions::default()));
	wait_until("generation start", || service.generation() > 0).await;
	assert!(!service.ensure_index(root.path().to_path_buf(), FilesystemOptions::default()));
}

#[tokio::test]
async fn query_ids_are_monotonic_per_generation() {
	let mut service = FsService::new();
	let root_a = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root_a.path().to_path_buf(), FilesystemOptions::default());
	wait_until("generation one", || service.generation() > 0).await;

	let first = service.query("main", 20).expect("query id");
	let second = service.query("lib", 20).expect("query id");
	assert!(second > first);

	let old_generation = service.generation();
	let root_b = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root_b.path().to_path_buf(), FilesystemOptions::default());
	wait_until("generation two", || service.generation() > old_generation).await;

	let next = service.query("src", 20).expect("query id");
	assert_eq!(next, 1);
}

#[tokio::test]
async fn drain_events_reports_actor_pushed_state_changes() {
	let mut service = FsService::new();
	let root = tempfile::tempdir().expect("must create tempdir");
	let file = root.path().join("main.rs");
	std::fs::write(&file, "fn main() {}\n").expect("must create file");

	service.ensure_index(root.path().to_path_buf(), FilesystemOptions::default());
	wait_until("event drain", || service.drain_events() > 0).await;
}

#[tokio::test]
async fn service_actor_restarts_and_recovers_after_failure() {
	let mut service = FsService::new();
	let before = service.service_restart_count();
	service.crash_for_test();

	wait_until("service restart", || service.service_restart_count() > before).await;

	let root = tempfile::tempdir().expect("must create tempdir");
	assert!(service.ensure_index(root.path().to_path_buf(), FilesystemOptions::default()));
	wait_until("service generation after restart", || service.generation() > 0).await;
}

#[tokio::test]
async fn rapid_query_burst_applies_latest_result_under_backpressure() {
	let mut service = FsService::new();
	let root = tempfile::tempdir().expect("must create tempdir");
	let src = root.path().join("src");
	std::fs::create_dir_all(&src).expect("must create src");
	for i in 0..1200usize {
		let path = src.join(format!("file_{i:04}.rs"));
		std::fs::write(path, "fn main() {}\n").expect("must create file");
	}

	service.ensure_index(root.path().to_path_buf(), FilesystemOptions::default());
	wait_until("index complete", || service.progress().complete).await;

	let last_query = "file_1199";
	for i in 0..512usize {
		let query = format!("file_{i:04}");
		let _ = service.query(query, 50);
	}
	let _ = service.query(last_query.to_string(), 50);

	wait_until("latest query result applied", || service.result_query() == last_query).await;
	assert!(service.results().len() <= 50);
}

#[tokio::test]
async fn shutdown_returns_completed_reports() {
	let service = FsService::new();
	let report = service
		.shutdown(ShutdownMode::Graceful {
			timeout: Duration::from_millis(200),
		})
		.await;

	assert!(report.service.completed);
	assert!(report.indexer.completed);
	assert!(report.search.completed);
	assert!(!report.service.timed_out);
	assert!(!report.indexer.timed_out);
	assert!(!report.search.timed_out);
}
