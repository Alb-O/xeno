use std::collections::BTreeMap;
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
	/// Resolves overlaps using a "last-wins" strategy with a sweep-line algorithm.
	/// Higher priority spans (those appearing later in the input) overwrite
	/// lower priority spans for overlapping regions.
	///
	/// # Parameters
	///
	/// - `spans`: The highlight spans with associated styles.
	pub fn new(spans: Vec<(HighlightSpan, Style)>) -> Self {
		if spans.is_empty() {
			return Self { spans: Vec::new() };
		}

		// Use a sweep-line algorithm to resolve overlaps correctly.
		#[derive(PartialEq, Eq, PartialOrd, Ord)]
		enum EventKind {
			End,
			Start,
		}

		let mut events = Vec::with_capacity(spans.len() * 2);
		for (i, (span, style)) in spans.iter().enumerate() {
			if span.start < span.end {
				events.push((span.start, EventKind::Start, i, style));
				events.push((span.end, EventKind::End, i, style));
			}
		}

		// Sort by position, then by kind (End before Start at same pos), then by priority.
		events.sort_unstable_by_key(|e| {
			(
				e.0,
				match e.1 {
					EventKind::End => 0,
					EventKind::Start => 1,
				},
				e.2,
			)
		});

		let mut flattened = Vec::new();
		let mut active: BTreeMap<usize, &Style> = BTreeMap::new();

		if events.is_empty() {
			return Self { spans: Vec::new() };
		}

		let mut last_pos = events[0].0;

		for (pos, kind, index, style) in events {
			if pos > last_pos
				&& let Some((_, active_style)) = active.last_key_value()
			{
				flattened.push((last_pos..pos, **active_style));
			}

			match kind {
				EventKind::Start => {
					active.insert(index, style);
				}
				EventKind::End => {
					active.remove(&index);
				}
			}
			last_pos = pos;
		}

		// Merge adjacent spans with identical styles.
		let mut merged: Vec<(Range<u32>, Style)> = Vec::with_capacity(flattened.len());
		for (range, style) in flattened {
			if let Some((last_range, last_style)) = merged.last_mut()
				&& *last_style == style
				&& last_range.end == range.start
			{
				last_range.end = range.end;
			} else {
				merged.push((range, style));
			}
		}

		Self { spans: merged }
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

#[cfg(test)]
mod tests;
