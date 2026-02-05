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
mod tests {
	use xeno_runtime_language::highlight::Highlight;
	use xeno_tui::style::Color;

	use super::*;

	fn s(start: u32, end: u32, color: Color) -> (HighlightSpan, Style) {
		(
			HighlightSpan {
				start,
				end,
				highlight: Highlight::new(0),
			},
			Style::default().fg(color),
		)
	}

	#[test]
	fn test_simple_overlap() {
		let spans = vec![s(0, 10, Color::Red), s(5, 15, Color::Blue)];
		let index = HighlightIndex::new(spans);
		// Expect [0,5] Red, [5,15] Blue (last wins)
		assert_eq!(index.spans.len(), 2);
		assert_eq!(index.spans[0].0, 0..5);
		assert_eq!(index.spans[0].1.fg, Some(Color::Red));
		assert_eq!(index.spans[1].0, 5..15);
		assert_eq!(index.spans[1].1.fg, Some(Color::Blue));
	}

	#[test]
	fn test_nested_overlap() {
		let spans = vec![
			s(0, 10, Color::Red),
			s(2, 8, Color::Blue),
			s(4, 6, Color::Green),
		];
		let index = HighlightIndex::new(spans);
		// Expect Red, Blue, Green, Blue, Red
		assert_eq!(index.spans.len(), 5);
		assert_eq!(index.spans[0].0, 0..2);
		assert_eq!(index.spans[0].1.fg, Some(Color::Red));
		assert_eq!(index.spans[1].0, 2..4);
		assert_eq!(index.spans[1].1.fg, Some(Color::Blue));
		assert_eq!(index.spans[2].0, 4..6);
		assert_eq!(index.spans[2].1.fg, Some(Color::Green));
		assert_eq!(index.spans[3].0, 6..8);
		assert_eq!(index.spans[3].1.fg, Some(Color::Blue));
		assert_eq!(index.spans[4].0, 8..10);
		assert_eq!(index.spans[4].1.fg, Some(Color::Red));
	}

	#[test]
	fn test_same_start_last_wins() {
		let spans = vec![s(0, 10, Color::Red), s(0, 5, Color::Blue)];
		let index = HighlightIndex::new(spans);
		// Since Blue comes later, it wins over Red for 0..5
		assert_eq!(index.spans.len(), 2);
		assert_eq!(index.spans[0].0, 0..5);
		assert_eq!(index.spans[0].1.fg, Some(Color::Blue));
		assert_eq!(index.spans[1].0, 5..10);
		assert_eq!(index.spans[1].1.fg, Some(Color::Red));
	}

	#[test]
	fn test_same_end_priority() {
		let spans = vec![s(0, 10, Color::Red), s(5, 10, Color::Blue)];
		let index = HighlightIndex::new(spans);
		// Expect [0,5] Red, [5,10] Blue
		assert_eq!(index.spans.len(), 2);
		assert_eq!(index.spans[0].0, 0..5);
		assert_eq!(index.spans[0].1.fg, Some(Color::Red));
		assert_eq!(index.spans[1].0, 5..10);
		assert_eq!(index.spans[1].1.fg, Some(Color::Blue));
	}

	#[test]
	fn test_end_start_adjacency() {
		let spans = vec![s(0, 5, Color::Red), s(5, 10, Color::Blue)];
		let index = HighlightIndex::new(spans);
		assert_eq!(index.spans.len(), 2);
		assert_eq!(index.spans[0].0, 0..5);
		assert_eq!(index.spans[1].0, 5..10);
	}

	#[test]
	fn test_adjacent_merging() {
		let spans = vec![s(0, 5, Color::Red), s(5, 10, Color::Red)];
		let index = HighlightIndex::new(spans);
		assert_eq!(index.spans.len(), 1);
		assert_eq!(index.spans[0].0, 0..10);
	}
}
