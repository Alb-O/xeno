//! Fuzzy-search primitives over indexed filesystem rows.
//!
//! Applies corpus deltas and runs ranked fuzzy matching with staleness
//! cancellation to produce bounded result sets.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::Instant;

use super::types::{IndexDelta, SearchData, SearchMsg, SearchRow};

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

pub(crate) fn apply_search_delta(data: &mut SearchData, delta: IndexDelta) {
	match delta {
		IndexDelta::Reset => data.files.clear(),
		IndexDelta::Replace(next) => *data = next,
		IndexDelta::Append(files) => data.files.extend(files.iter().cloned()),
	}
}

pub(crate) fn run_search_query(generation: u64, id: u64, query: &str, limit: usize, data: &SearchData, latest_query_id: &AtomicU64) -> Option<SearchMsg> {
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
