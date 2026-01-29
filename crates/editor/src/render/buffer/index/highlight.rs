use std::ops::Range;

use xeno_runtime_language::highlight::HighlightSpan;
use xeno_tui::style::Style;

#[derive(Debug, Clone)]
pub struct HighlightIndex {
	spans: Vec<(Range<u32>, Style)>,
}

impl HighlightIndex {
	/// Creates a new highlight index from a collection of spans.
	///
	/// Flatten overlapping spans by clipping them. Preserves the last span
	/// (most specific/highest priority) when multiple spans start at the same
	/// position or overlap.
	///
	/// # Parameters
	///
	/// - `spans`: The highlight spans with associated styles.
	pub fn new(spans: Vec<(HighlightSpan, Style)>) -> Self {
		let mut index_spans: Vec<_> = spans
			.into_iter()
			.map(|(s, style)| (s.start..s.end, style))
			.collect();

		// Sort primarily by start, then by end (longer spans first).
		index_spans
			.sort_by(|(r1, _), (r2, _)| r1.start.cmp(&r2.start).then_with(|| r1.end.cmp(&r2.end)));

		let mut flattened: Vec<(Range<u32>, Style)> = Vec::with_capacity(index_spans.len());
		for (range, style) in index_spans {
			if let Some((last_range, _)) = flattened.last_mut()
				&& range.start < last_range.end
			{
				// Overlap detected.
				if range.start == last_range.start {
					// Both start at the same point. Replace the previous one (last-wins).
					*last_range = range;
					continue;
				} else {
					// New span starts after previous but before it ends. Clip the previous span.
					last_range.end = range.start;
					if last_range.start >= last_range.end {
						flattened.pop();
					}
				}
			}
			flattened.push((range, style));
		}

		// Final dedup of any now-empty or identical spans after clipping.
		flattened.retain(|(r, _)| r.start < r.end);
		flattened.dedup_by(|(r2, _), (r1, _)| r1.start == r2.start && r1.end == r2.end);

		Self { spans: flattened }
	}

	pub fn style_at(&self, byte_pos: u32) -> Option<Style> {
		let idx = self
			.spans
			.binary_search_by(|(r, _)| {
				if byte_pos < r.start {
					std::cmp::Ordering::Greater
				} else if byte_pos >= r.end {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Equal
				}
			})
			.ok()?;

		Some(self.spans[idx].1)
	}
}
