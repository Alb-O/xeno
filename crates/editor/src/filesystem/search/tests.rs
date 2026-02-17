use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;

use super::{RankedMatch, build_search_rows, spawn_search_worker};
use crate::filesystem::{FileRow, IndexDelta, SearchCmd, SearchData, SearchMsg};

fn seed_data() -> SearchData {
	SearchData {
		root: None,
		files: vec![
			FileRow::new(Arc::<str>::from("src/main.rs")),
			FileRow::new(Arc::<str>::from("src/lib.rs")),
			FileRow::new(Arc::<str>::from("README.md")),
		],
	}
}

#[test]
fn worker_returns_matches_for_latest_query() {
	let runtime = xeno_worker::WorkerRuntime::new();
	let (command_tx, result_rx, latest_query_id) = spawn_search_worker(&runtime, 1, SearchData::default());

	command_tx
		.send(SearchCmd::Update {
			generation: 1,
			delta: IndexDelta::Replace(seed_data()),
		})
		.expect("send update");

	latest_query_id.store(1, AtomicOrdering::Release);
	command_tx
		.send(SearchCmd::Query {
			generation: 1,
			id: 1,
			query: "main".to_string(),
			limit: 10,
		})
		.expect("send query");

	let result = result_rx.recv_timeout(Duration::from_secs(2)).expect("receive search result");
	let SearchMsg::Result { id, rows, .. } = result;
	assert_eq!(id, 1);
	assert!(rows.iter().any(|row| row.path.as_ref() == "src/main.rs"));
}

#[test]
fn worker_suppresses_stale_query_results() {
	let runtime = xeno_worker::WorkerRuntime::new();
	let (command_tx, result_rx, latest_query_id) = spawn_search_worker(&runtime, 1, SearchData::default());

	command_tx
		.send(SearchCmd::Update {
			generation: 1,
			delta: IndexDelta::Replace(seed_data()),
		})
		.expect("send update");

	latest_query_id.store(2, AtomicOrdering::Release);
	command_tx
		.send(SearchCmd::Query {
			generation: 1,
			id: 1,
			query: "lib".to_string(),
			limit: 10,
		})
		.expect("send stale query");

	command_tx
		.send(SearchCmd::Query {
			generation: 1,
			id: 2,
			query: "main".to_string(),
			limit: 10,
		})
		.expect("send latest query");

	let result = result_rx.recv_timeout(Duration::from_secs(2)).expect("receive latest search result");
	let SearchMsg::Result { id, .. } = result;
	assert_eq!(id, 2);
}

#[test]
fn phase_two_reconstruction_aborts_stale_query() {
	let latest_query_id = AtomicU64::new(2);
	let config = crate::completion::frizbee_config_for_query("main");
	let mut heap = BinaryHeap::new();
	heap.push(std::cmp::Reverse(RankedMatch {
		score: 100,
		index: 0,
		path: Arc::<str>::from("src/main.rs"),
	}));

	let rows = build_search_rows("main", 1, &config, heap, &latest_query_id);
	assert!(rows.is_none());
}
