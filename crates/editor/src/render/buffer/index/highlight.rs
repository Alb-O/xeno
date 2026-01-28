use std::ops::Range;

use xeno_runtime_language::highlight::HighlightSpan;
use xeno_tui::style::Style;

#[derive(Debug, Clone)]
pub struct HighlightIndex {
	spans: Vec<(Range<u32>, Style)>,
}

impl HighlightIndex {
	pub fn new(spans: Vec<(HighlightSpan, Style)>) -> Self {
		let mut index_spans: Vec<_> = spans
			.into_iter()
			.map(|(s, style)| (s.start..s.end, style))
			.collect();
		index_spans.sort_by_key(|(r, _)| r.start);

		// Debug validation: ensure spans are non-overlapping
		// Overlapping spans would break binary_search_by correctness
		#[cfg(debug_assertions)]
		{
			for window in index_spans.windows(2) {
				let (r1, _) = &window[0];
				let (r2, _) = &window[1];
				debug_assert!(
					r1.end <= r2.start,
					"Overlapping highlight spans detected: {:?} and {:?}",
					r1,
					r2
				);
			}
		}

		Self { spans: index_spans }
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
