use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use super::{RankedMatch, apply_search_delta, build_search_rows, run_search_query};
use crate::filesystem::types::{FileRow, IndexDelta, SearchData, SearchMsg};

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
fn run_search_query_returns_matches_for_latest_query() {
	let mut data = SearchData::default();
	apply_search_delta(&mut data, IndexDelta::Replace(seed_data()));
	let latest_query_id = AtomicU64::new(1);

	let result = run_search_query(1, 1, "main", 10, &data, &latest_query_id).expect("receive search result");
	let SearchMsg::Result { id, rows, .. } = result;
	assert_eq!(id, 1);
	assert!(rows.iter().any(|row| row.path.as_ref() == "src/main.rs"));
}

#[test]
fn run_search_query_suppresses_stale_query_results() {
	let mut data = SearchData::default();
	apply_search_delta(&mut data, IndexDelta::Replace(seed_data()));
	let latest_query_id = AtomicU64::new(2);

	let stale = run_search_query(1, 1, "lib", 10, &data, &latest_query_id);
	assert!(stale.is_none());

	let latest = run_search_query(1, 2, "main", 10, &data, &latest_query_id).expect("latest query result");
	let SearchMsg::Result { id, .. } = latest;
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
