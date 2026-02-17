use std::sync::Arc;
use std::time::Duration;

use tokio::time::{sleep, timeout};

use super::FsService;
use crate::filesystem::types::{FileRow, IndexMsg, IndexUpdate, ProgressSnapshot, SearchMsg, SearchRow};

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

/// Must ignore stale-generation index updates.
///
/// * Enforced in: `FsServiceActor::apply_index_msg`
/// * Failure symptom: old worker generation overwrites current index state.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_index_generation_ignored() {
	let mut service = FsService::new();
	let stale_generation = service.generation();

	let root = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root.path().to_path_buf(), crate::filesystem::FilesystemOptions::default());
	wait_until("generation advance", || service.generation() > stale_generation).await;

	service.inject_index_msg(IndexMsg::Update(IndexUpdate {
		generation: stale_generation,
		reset: false,
		files: vec![FileRow::new(Arc::<str>::from("src/lib.rs"))].into(),
		progress: ProgressSnapshot {
			indexed_files: 1,
			total_files: Some(1),
			complete: false,
		},
		cached_data: None,
	}));

	sleep(Duration::from_millis(25)).await;
	assert!(service.data().files.is_empty());
}

/// Must reset query/progress/result state when beginning a new generation.
///
/// * Enforced in: `FsServiceActor::begin_new_generation`
/// * Failure symptom: new index run starts with stale query results/progress counters.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_begin_new_generation_resets_observable_state() {
	let mut service = FsService::new();
	let root_a = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root_a.path().to_path_buf(), crate::filesystem::FilesystemOptions::default());
	wait_until("first generation", || service.generation() > 0).await;
	let generation = service.generation();

	service.inject_search_msg(SearchMsg::Result {
		generation,
		id: 3,
		query: "main".to_string(),
		rows: vec![SearchRow {
			path: Arc::<str>::from("src/main.rs"),
			score: 10,
			match_indices: None,
		}]
		.into(),
		scanned: 1,
		elapsed_ms: 1,
	});

	wait_until("result visible", || service.result_query() == "main").await;
	assert_eq!(service.results().len(), 1);

	let root_b = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root_b.path().to_path_buf(), crate::filesystem::FilesystemOptions::default());
	wait_until("second generation", || service.generation() > generation).await;

	let progress = service.progress();
	if progress.complete {
		assert_eq!(progress.indexed_files, progress.total_files.unwrap_or_default());
	}
	assert_eq!(service.result_query(), "");
	assert!(service.results().is_empty());
}

/// Must ignore stale-generation search results.
///
/// * Enforced in: `FsServiceActor::apply_search_msg`
/// * Failure symptom: outdated search result list replaces active query output.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_search_generation_ignored() {
	let mut service = FsService::new();
	let stale_generation = service.generation();

	let root = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root.path().to_path_buf(), crate::filesystem::FilesystemOptions::default());
	wait_until("generation advance", || service.generation() > stale_generation).await;

	service.inject_search_msg(SearchMsg::Result {
		generation: stale_generation,
		id: 7,
		query: "main".to_string(),
		rows: vec![SearchRow {
			path: Arc::<str>::from("src/main.rs"),
			score: 42,
			match_indices: Some(vec![0, 1]),
		}]
		.into(),
		scanned: 100,
		elapsed_ms: 3,
	});

	sleep(Duration::from_millis(25)).await;
	assert!(service.results().is_empty());
}

/// Must emit pushed snapshot events whenever observable state changes.
///
/// * Enforced in: `FsServiceActor::handle` change gate + `ctx.emit(FsServiceEvt::SnapshotChanged)`
/// * Failure symptom: UI does not refresh after indexing/query snapshot updates until unrelated runtime work occurs.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_snapshot_changes_emit_events() {
	let mut service = FsService::new();
	assert_eq!(service.drain_events(), 0);

	let root = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root.path().to_path_buf(), crate::filesystem::FilesystemOptions::default());
	wait_until("snapshot event", || service.drain_events() > 0).await;
}

/// Must expose query submission as enqueue-success status without handle-owned query IDs.
///
/// * Enforced in: `FsService::query`
/// * Failure symptom: handle mirrors actor query IDs and diverges from actor-owned sequencing.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_query_api_returns_enqueue_success_only() {
	let mut service = FsService::new();
	assert!(!service.query("main", 20));

	let root = tempfile::tempdir().expect("must create tempdir");
	service.ensure_index(root.path().to_path_buf(), crate::filesystem::FilesystemOptions::default());
	wait_until("generation advance", || service.generation() > 0).await;
	assert!(service.query("main", 20));
}
