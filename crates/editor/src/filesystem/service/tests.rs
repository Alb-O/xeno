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

#[test]
fn pump_applies_current_generation_index_updates() {
	let (tx, rx) = mpsc::channel();
	let mut service = FsService::new();
	service.set_index_receiver(rx);

	let files: Arc<[FileRow]> = vec![FileRow::new(Arc::<str>::from("src/main.rs"))].into();
	tx.send(IndexMsg::Update(IndexUpdate {
		generation: service.generation(),
		kind: IndexKind::Live,
		reset: false,
		files,
		progress: ProgressSnapshot {
			indexed_files: 1,
			total_files: Some(2),
			complete: false,
		},
		cached_data: None,
	}))
	.unwrap();

	assert!(service.pump(budget()));
	assert_eq!(service.data().files.len(), 1);
	assert_eq!(service.progress().indexed_files, 1);
}

#[test]
fn pump_ignores_stale_generation_messages() {
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

#[test]
fn pump_applies_current_generation_search_results() {
	let (tx, rx) = mpsc::channel();
	let mut service = FsService::new();
	service.set_search_receiver(rx);

	tx.send(SearchMsg::Result {
		generation: service.generation(),
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

	assert!(service.pump(budget()));
	assert_eq!(service.result_query(), "main");
	assert_eq!(service.results().len(), 1);
}
