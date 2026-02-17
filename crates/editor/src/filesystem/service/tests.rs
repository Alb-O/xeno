use std::time::Duration;

use tokio::time::{sleep, timeout};

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
async fn pump_reports_actor_pushed_state_changes() {
	let mut service = FsService::new();
	let root = tempfile::tempdir().expect("must create tempdir");
	let file = root.path().join("main.rs");
	std::fs::write(&file, "fn main() {}\n").expect("must create file");

	service.ensure_index(root.path().to_path_buf(), FilesystemOptions::default());
	wait_until("pump changed", || service.pump()).await;
}
