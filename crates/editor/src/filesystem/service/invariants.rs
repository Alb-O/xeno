use std::sync::{Arc, mpsc};
use std::time::Duration;

use super::FsService;
use crate::filesystem::{FileRow, IndexKind, IndexMsg, IndexUpdate, ProgressSnapshot, PumpBudget, SearchMsg, SearchRow};

fn budget() -> PumpBudget {
	PumpBudget {
		max_index_msgs: 32,
		max_search_msgs: 8,
		max_time: Duration::from_millis(10),
	}
}

/// Must ignore stale-generation index updates.
///
/// * Enforced in: `FsService::apply_index_msg`
/// * Failure symptom: old worker generation overwrites current index state.
#[cfg_attr(test, test)]
pub(crate) fn test_stale_index_generation_ignored() {
	let (tx, rx) = mpsc::channel();
	let mut service = FsService::new();
	service.set_index_receiver(rx);
	let stale_generation = service.generation();
	service.begin_new_generation();

	tx.send(IndexMsg::Update(IndexUpdate {
		generation: stale_generation,
		kind: IndexKind::Live,
		reset: false,
		files: vec![FileRow::new(Arc::<str>::from("src/lib.rs"))].into(),
		progress: ProgressSnapshot {
			indexed_files: 1,
			total_files: Some(1),
			complete: false,
		},
		cached_data: None,
	}))
	.unwrap();

	assert!(!service.pump(budget()));
	assert!(service.data().files.is_empty());
}

/// Must reset query/progress/result state when beginning a new generation.
///
/// * Enforced in: `FsService::begin_new_generation`
/// * Failure symptom: new index run starts with stale query results/progress counters.
#[cfg_attr(test, test)]
pub(crate) fn test_begin_new_generation_resets_observable_state() {
	let (index_tx, index_rx) = mpsc::channel();
	let (search_tx, search_rx) = mpsc::channel();
	let mut service = FsService::new();
	service.set_index_receiver(index_rx);
	service.set_search_receiver(search_rx);

	index_tx
		.send(IndexMsg::Update(IndexUpdate {
			generation: service.generation(),
			kind: IndexKind::Live,
			reset: false,
			files: vec![FileRow::new(Arc::<str>::from("src/main.rs"))].into(),
			progress: ProgressSnapshot {
				indexed_files: 1,
				total_files: Some(2),
				complete: false,
			},
			cached_data: None,
		}))
		.unwrap();

	search_tx
		.send(SearchMsg::Result {
			generation: service.generation(),
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
		})
		.unwrap();
	assert!(service.pump(budget()));
	assert_eq!(service.result_query(), "main");
	assert_eq!(service.results().len(), 1);

	service.begin_new_generation();

	let progress = service.progress();
	assert_eq!(progress.indexed_files, 0);
	assert_eq!(progress.total_files, None);
	assert!(!progress.complete);
	assert_eq!(service.result_query(), "");
	assert!(service.results().is_empty());
}

/// Must ignore stale-generation search results.
///
/// * Enforced in: `FsService::apply_search_msg`
/// * Failure symptom: outdated search result list replaces active query output.
#[cfg_attr(test, test)]
pub(crate) fn test_stale_search_generation_ignored() {
	let (tx, rx) = mpsc::channel();
	let mut service = FsService::new();
	let stale_generation = service.generation();
	service.begin_new_generation();
	service.set_search_receiver(rx);

	tx.send(SearchMsg::Result {
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
	})
	.unwrap();

	assert!(!service.pump(budget()));
	assert!(service.results().is_empty());
}
