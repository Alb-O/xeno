use std::sync::Arc;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::time::Duration;

use super::spawn_search_worker;
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
	let (command_tx, result_rx, latest_query_id) = spawn_search_worker(1, SearchData::default());

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
	let (command_tx, result_rx, latest_query_id) = spawn_search_worker(1, SearchData::default());

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
