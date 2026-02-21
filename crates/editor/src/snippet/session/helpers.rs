use std::collections::BTreeMap;
use std::ops::Range as StdRange;

use xeno_primitives::{Change, CharIdx, Range, Selection};

use super::ActiveMode;

pub(super) fn tabstop_order(tabstops: &BTreeMap<u32, Vec<StdRange<CharIdx>>>) -> Vec<u32> {
	let mut order: Vec<u32> = tabstops.keys().copied().filter(|idx| *idx > 0).collect();
	if tabstops.contains_key(&0) {
		order.push(0);
	}
	order
}

pub(super) fn active_mode_for_tabstop(tabstops: &BTreeMap<u32, Vec<StdRange<CharIdx>>>, index: u32) -> ActiveMode {
	if tabstops.get(&index).is_some_and(|ranges| ranges.iter().any(|range| range.start < range.end)) {
		ActiveMode::Replace
	} else {
		ActiveMode::Insert
	}
}

pub(super) fn compute_span(tabstops: &BTreeMap<u32, Vec<StdRange<CharIdx>>>) -> Option<StdRange<CharIdx>> {
	let mut min_start: Option<CharIdx> = None;
	let mut max_end: Option<CharIdx> = None;

	for ranges in tabstops.values() {
		for range in ranges {
			min_start = Some(min_start.map_or(range.start, |current| current.min(range.start)));
			max_end = Some(max_end.map_or(range.end, |current| current.max(range.end)));
		}
	}

	Some(min_start?..max_end?)
}

pub(super) fn has_overlapping_ranges(ranges: &[StdRange<CharIdx>]) -> bool {
	ranges.windows(2).any(|pair| pair[0].end > pair[1].start)
}

pub(super) fn has_overlapping_changes(changes: &[Change]) -> bool {
	changes.windows(2).any(|pair| pair[1].start < pair[0].end)
}

pub(super) fn primary_relative_range(ranges: &[StdRange<usize>]) -> Option<StdRange<usize>> {
	ranges.iter().min_by_key(|range| (range.start, range.end)).cloned()
}

pub(super) fn normalize_ranges(mut ranges: Vec<StdRange<CharIdx>>) -> Vec<StdRange<CharIdx>> {
	ranges.sort_by_key(|range| (range.start, range.end));
	let mut out: Vec<StdRange<CharIdx>> = Vec::with_capacity(ranges.len());

	for range in ranges {
		if let Some(last) = out.last_mut() {
			if range.start == last.start && range.end == last.end {
				continue;
			}
			if range.start < last.end {
				last.end = last.end.max(range.end);
				continue;
			}
		}
		out.push(range);
	}

	out
}

pub(super) fn to_selection_range(range: StdRange<CharIdx>) -> Range {
	if range.start == range.end {
		Range::point(range.start)
	} else {
		Range::from_exclusive(range.start, range.end)
	}
}

pub(super) fn selection_from_points(points: Vec<CharIdx>) -> Option<Selection> {
	let mut points = points;
	points.sort_unstable();
	points.dedup();
	let primary = points.first().copied()?;
	Some(Selection::new(Range::point(primary), points.into_iter().skip(1).map(Range::point)))
}
