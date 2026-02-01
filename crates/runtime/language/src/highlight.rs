//! Syntax highlighting types and utilities.
//!
//! This module bridges tree-sitter highlighting with Xeno's theme system,
//! providing the `Highlighter` iterator that produces highlight events.

use std::ops::{Bound, RangeBounds};

use ropey::RopeSlice;
pub use tree_house::highlighter::{Highlight, HighlightEvent};
use xeno_primitives::Style;

use crate::loader::LanguageLoader;

/// Maps highlight captures to styles.
///
/// Pre-resolved styles indexed by `Highlight`. This is the bridge between
/// tree-sitter capture names (from .scm files) and Xeno's theme system.
#[derive(Debug, Clone)]
pub struct HighlightStyles {
	/// Pre-resolved styles indexed by Highlight index.
	styles: Vec<Style>,
}

impl HighlightStyles {
	/// Creates a new highlight styles mapper by resolving all scopes upfront.
	///
	/// # Parameters
	/// - `scopes`: List of recognized scope names in order
	/// - `resolver`: Function that resolves a scope name to a style
	pub fn new<F>(scopes: &[impl AsRef<str>], resolver: F) -> Self
	where
		F: Fn(&str) -> Style,
	{
		let styles = scopes.iter().map(|s| resolver(s.as_ref())).collect();
		Self { styles }
	}

	/// Returns the number of highlight styles.
	pub fn len(&self) -> usize {
		self.styles.len()
	}

	/// Returns true if there are no highlight styles.
	pub fn is_empty(&self) -> bool {
		self.styles.is_empty()
	}

	/// Resolves a highlight index to a style.
	#[inline]
	pub fn style_for_highlight(&self, highlight: Highlight) -> Style {
		self.styles
			.get(highlight.idx())
			.copied()
			.unwrap_or_default()
	}
}

/// Iterator that produces syntax highlight spans.
///
/// This wraps tree-house's highlighter to provide an ergonomic `Iterator` API
/// that yields `HighlightSpan` items directly, avoiding allocation.
pub struct Highlighter<'a> {
	/// The underlying tree-house highlighter.
	inner: tree_house::highlighter::Highlighter<'a, 'a, LanguageLoader>,
	/// Byte offset where highlighting should stop.
	end_byte: u32,
	/// Current span start position.
	current_start: u32,
	/// The active highlight (innermost scope).
	current_highlight: Option<Highlight>,
}

impl<'a> Highlighter<'a> {
	/// Creates a new highlighter for the given syntax tree and range.
	pub fn new(
		syntax: &'a tree_house::Syntax,
		source: RopeSlice<'a>,
		loader: &'a LanguageLoader,
		range: impl RangeBounds<u32>,
	) -> Self {
		let start = match range.start_bound() {
			Bound::Included(&n) => n,
			Bound::Excluded(&n) => n + 1,
			Bound::Unbounded => 0,
		};
		let end = match range.end_bound() {
			Bound::Included(&n) => n + 1,
			Bound::Excluded(&n) => n,
			Bound::Unbounded => source.len_bytes() as u32,
		};

		let inner = tree_house::highlighter::Highlighter::new(syntax, source, loader, start..end);

		Self {
			current_start: inner.next_event_offset(),
			inner,
			end_byte: end,
			current_highlight: None,
		}
	}

	/// Returns the byte offset where the next event will occur.
	pub fn next_event_offset(&self) -> u32 {
		self.inner.next_event_offset()
	}

	/// Returns true if there are more events to process.
	pub fn is_done(&self) -> bool {
		self.next_event_offset() >= self.end_byte
	}

	/// Collects all highlight spans into a vector.
	///
	/// Convenience wrapper; prefer iterating directly to avoid allocation.
	pub fn collect_spans(self) -> Vec<HighlightSpan> {
		self.collect()
	}

	/// Closes the current span at `event_start` if there is an active highlight
	/// and the region is non-empty.
	fn close_span(&self, event_start: u32) -> Option<HighlightSpan> {
		self.current_highlight.and_then(|h| {
			(self.current_start < event_start).then_some(HighlightSpan {
				start: self.current_start,
				end: event_start,
				highlight: h,
			})
		})
	}
}

/// Advances through tree-house [`HighlightEvent`]s, emitting one
/// [`HighlightSpan`] per contiguous styled region.
///
/// Each event marks a boundary where the highlight stack changes. The iterator
/// closes the previous region (if any) and opens a new one:
///
/// - [`HighlightEvent::Push`]: A new scope is entered. Only updates the active
///   highlight when the push carries a non-empty highlight — an empty push
///   (e.g. entering an injection layer) preserves the parent scope's style.
/// - [`HighlightEvent::Refresh`]: The highlight stack was restructured.
///   Unconditionally replaces the active highlight with the new stack top.
///
/// After the inner iterator is exhausted, a final span is emitted covering any
/// remaining bytes up to `end_byte`.
impl<'a> Iterator for Highlighter<'a> {
	type Item = HighlightSpan;

	fn next(&mut self) -> Option<Self::Item> {
		while self.inner.next_event_offset() < self.end_byte {
			let event_start = self.inner.next_event_offset();
			let (event, mut highlights) = self.inner.advance();
			let new_highlight = highlights.next_back();

			let span = self.close_span(event_start);

			self.current_start = event_start;
			match event {
				HighlightEvent::Push => {
					if new_highlight.is_some() {
						self.current_highlight = new_highlight;
					}
				}
				HighlightEvent::Refresh => {
					self.current_highlight = new_highlight;
				}
			}

			if span.is_some() {
				return span;
			}
		}

		if let Some(h) = self.current_highlight.take() {
			let offset = self.inner.next_event_offset().min(self.end_byte);
			if self.current_start < offset {
				return Some(HighlightSpan {
					start: self.current_start,
					end: offset,
					highlight: h,
				});
			}
		}

		None
	}
}

/// A span of text with a specific highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
	/// Start byte offset (inclusive).
	pub start: u32,
	/// End byte offset (exclusive).
	pub end: u32,
	/// The highlight to apply.
	pub highlight: Highlight,
}

impl HighlightSpan {
	/// Returns the byte range.
	pub fn range(&self) -> std::ops::Range<u32> {
		self.start..self.end
	}

	/// Returns the length in bytes.
	pub fn len(&self) -> u32 {
		self.end - self.start
	}

	/// Returns true if the span is empty.
	pub fn is_empty(&self) -> bool {
		self.start >= self.end
	}
}

#[cfg(test)]
mod tests {
	use xeno_primitives::Color;

	use super::*;

	#[test]
	fn test_highlight_styles() {
		let scopes = ["keyword", "string"];

		let styles = HighlightStyles::new(&scopes, |scope| match scope {
			"keyword" => Style::new().fg(Color::Red),
			"string" => Style::new().fg(Color::Green),
			_ => Style::new(),
		});

		assert_eq!(styles.len(), 2);
		assert_eq!(
			styles.style_for_highlight(Highlight::new(0)),
			Style::new().fg(Color::Red)
		);
		assert_eq!(
			styles.style_for_highlight(Highlight::new(1)),
			Style::new().fg(Color::Green)
		);
	}

	#[test]
	fn test_highlight_span() {
		let span = HighlightSpan {
			start: 10,
			end: 20,
			highlight: Highlight::new(0),
		};

		assert_eq!(span.range(), 10..20);
		assert_eq!(span.len(), 10);
		assert!(!span.is_empty());
	}
}
