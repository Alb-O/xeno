//! Background fuzzy-search worker over indexed filesystem rows.
//!
//! Processes command/query messages, applies index deltas, runs ranked fuzzy
//! matching with staleness cancellation, and returns bounded result sets.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread;
use std::time::Instant;

use super::types::{IndexDelta, SearchCmd, SearchData, SearchMsg, SearchRow};

const STALE_CHECK_INTERVAL: usize = 256;

#[derive(Clone, Debug)]
struct RankedMatch {
	score: u16,
	index: usize,
	path: Arc<str>,
}

impl PartialEq for RankedMatch {
	fn eq(&self, other: &Self) -> bool {
		self.score == other.score && self.index == other.index
	}
}

impl Eq for RankedMatch {}

impl Ord for RankedMatch {
	fn cmp(&self, other: &Self) -> Ordering {
		(self.score as i32).cmp(&(other.score as i32)).then_with(|| other.index.cmp(&self.index))
	}
}

impl PartialOrd for RankedMatch {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

pub fn spawn_search_worker(generation: u64, data: SearchData) -> (Sender<SearchCmd>, Receiver<SearchMsg>, Arc<AtomicU64>) {
	let (command_tx, command_rx) = mpsc::channel();
	let (result_tx, result_rx) = mpsc::sync_channel(1);
	let latest_query_id = Arc::new(AtomicU64::new(0));
	let worker_latest = Arc::clone(&latest_query_id);

	thread::spawn(move || worker_loop(generation, data, command_rx, result_tx, worker_latest));

	(command_tx, result_rx, latest_query_id)
}

fn worker_loop(generation: u64, mut data: SearchData, command_rx: Receiver<SearchCmd>, result_tx: SyncSender<SearchMsg>, latest_query_id: Arc<AtomicU64>) {
	while let Ok(command) = command_rx.recv() {
		match command {
			SearchCmd::Update {
				generation: command_generation,
				delta,
			} => {
				if command_generation != generation {
					continue;
				}
				apply_delta(&mut data, delta);
			}
			SearchCmd::Query {
				generation: command_generation,
				id,
				query,
				limit,
			} => {
				if command_generation != generation {
					continue;
				}
				if should_abort(id, latest_query_id.as_ref()) {
					continue;
				}

				let Some(result) = run_query(generation, id, &query, limit, &data, latest_query_id.as_ref()) else {
					continue;
				};
				if should_abort(id, latest_query_id.as_ref()) {
					continue;
				}

				match result_tx.try_send(result) {
					Ok(()) => {}
					Err(TrySendError::Full(_)) => {}
					Err(TrySendError::Disconnected(_)) => break,
				}
			}
			SearchCmd::Shutdown {
				generation: command_generation,
			} => {
				if command_generation == generation {
					break;
				}
			}
		}
	}
}

fn apply_delta(data: &mut SearchData, delta: IndexDelta) {
	match delta {
		IndexDelta::Reset => data.files.clear(),
		IndexDelta::Replace(next) => *data = next,
		IndexDelta::Append(files) => data.files.extend(files.iter().cloned()),
	}
}

fn run_query(generation: u64, id: u64, query: &str, limit: usize, data: &SearchData, latest_query_id: &AtomicU64) -> Option<SearchMsg> {
	let start = Instant::now();
	if limit == 0 {
		return Some(SearchMsg::Result {
			generation,
			id,
			query: query.to_string(),
			rows: Arc::from(Vec::<SearchRow>::new()),
			scanned: 0,
			elapsed_ms: start.elapsed().as_millis() as u64,
		});
	}

	let config = crate::completion::frizbee_config_for_query(query);
	let scorer = xeno_matcher::ScoreMatcher::new(query, &config);
	let mut heap: BinaryHeap<std::cmp::Reverse<RankedMatch>> = BinaryHeap::new();
	let mut scanned = 0usize;

	// Phase 1: score-only scan to find top-K candidates via SIMD (fast)
	for (idx, file) in data.files.iter().enumerate() {
		scanned += 1;
		if scanned.is_multiple_of(STALE_CHECK_INTERVAL) && should_abort(id, latest_query_id) {
			return None;
		}

		let Some((score, _exact)) = scorer.score(file.path.as_ref()) else {
			continue;
		};

		let candidate = RankedMatch {
			score,
			index: idx,
			path: Arc::clone(&file.path),
		};

		if heap.len() < limit {
			heap.push(std::cmp::Reverse(candidate));
			continue;
		}

		if let Some(std::cmp::Reverse(worst)) = heap.peek()
			&& candidate > *worst
		{
			heap.pop();
			heap.push(std::cmp::Reverse(candidate));
		}
	}

	if should_abort(id, latest_query_id) {
		return None;
	}

	// Phase 2: compute match indices only for top-K results (reference SW, slow)
	let rows = build_search_rows(query, id, &config, heap, latest_query_id)?;

	if should_abort(id, latest_query_id) {
		return None;
	}

	Some(SearchMsg::Result {
		generation,
		id,
		query: query.to_string(),
		rows: rows.into(),
		scanned,
		elapsed_ms: start.elapsed().as_millis() as u64,
	})
}

fn build_search_rows(
	query: &str,
	id: u64,
	config: &xeno_matcher::Config,
	heap: BinaryHeap<std::cmp::Reverse<RankedMatch>>,
	latest_query_id: &AtomicU64,
) -> Option<Vec<SearchRow>> {
	let mut rows = Vec::with_capacity(heap.len());
	for std::cmp::Reverse(entry) in heap {
		if should_abort(id, latest_query_id) {
			return None;
		}

		let match_indices =
			xeno_matcher::match_indices(query, entry.path.as_ref(), config).and_then(|mi| if mi.indices.is_empty() { None } else { Some(mi.indices) });
		rows.push(SearchRow {
			path: entry.path,
			score: entry.score as i32,
			match_indices,
		});
	}
	if should_abort(id, latest_query_id) {
		return None;
	}

	rows.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.as_ref().cmp(b.path.as_ref())));

	if should_abort(id, latest_query_id) {
		return None;
	}
	Some(rows)
}

fn should_abort(id: u64, latest_query_id: &AtomicU64) -> bool {
	latest_query_id.load(AtomicOrdering::Acquire) != id
}

#[cfg(test)]
mod tests;
