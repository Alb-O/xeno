//! Syntax highlighting types and utilities.
//!
//! This module bridges tree-sitter highlighting with Tome's theme system,
//! providing the `Highlighter` iterator that produces highlight events.

use std::ops::{Bound, RangeBounds};

use ratatui::style::Style;
use ropey::RopeSlice;
use crate::loader::LanguageLoader;

// Re-export tree-house highlight types for convenience.
pub use tree_house::highlighter::{Highlight, HighlightEvent};

/// Maps highlight captures to styles.
///
/// This is the bridge between tree-sitter capture names (from .scm files)
/// and Tome's theme system.
pub struct HighlightStyles {
	/// Ordered list of scope names that we recognize.
	/// The index in this list corresponds to the Highlight index.
	scopes: Vec<String>,

	/// Resolver function that maps scope name to style.
	resolver: Box<dyn Fn(&str) -> Style + Send + Sync>,
}

impl HighlightStyles {
	/// Creates a new highlight styles mapper.
	///
	/// # Parameters
	/// - `scopes`: List of recognized scope names in order
	/// - `resolver`: Function that resolves a scope name to a style
	pub fn new<F>(scopes: Vec<String>, resolver: F) -> Self
	where
		F: Fn(&str) -> Style + Send + Sync + 'static,
	{
		Self {
			scopes,
			resolver: Box::new(resolver),
		}
	}

	/// Returns the list of recognized scopes.
	pub fn scopes(&self) -> &[String] {
		&self.scopes
	}

	/// Resolves a highlight index to a style.
	pub fn style_for_highlight(&self, highlight: Highlight) -> Style {
		self.scopes
			.get(highlight.idx())
			.map(|scope| (self.resolver)(scope))
			.unwrap_or_default()
	}

	/// Resolves a scope name to a style.
	pub fn style_for_scope(&self, scope: &str) -> Style {
		(self.resolver)(scope)
	}
}

impl std::fmt::Debug for HighlightStyles {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HighlightStyles")
			.field("scopes", &self.scopes)
			.field("resolver", &"<fn>")
			.finish()
	}
}

/// Iterator that produces syntax highlight spans.
///
/// This wraps tree-house's highlighter to provide an ergonomic `Iterator` API
/// that yields `HighlightSpan` items directly, avoiding allocation.
pub struct Highlighter<'a> {
	inner: tree_house::highlighter::Highlighter<'a, 'a, LanguageLoader>,
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

		let inner =
			tree_house::highlighter::Highlighter::new(syntax, source, loader, start..end);

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
	/// This is a convenience method; prefer iterating directly for efficiency.
	pub fn collect_spans(self) -> Vec<HighlightSpan> {
		self.collect()
	}
}

impl<'a> Iterator for Highlighter<'a> {
	type Item = HighlightSpan;

	fn next(&mut self) -> Option<Self::Item> {
		// Keep advancing until we have a span to emit or we're done
		while self.inner.next_event_offset() < self.end_byte {
			let (event, highlights) = self.inner.advance();
			let new_highlight = highlights.last();
			let offset = self.inner.next_event_offset();

			// Check if we need to emit a span for the previous highlight
			let span = match event {
				HighlightEvent::Push | HighlightEvent::Refresh => {
					let span = self.current_highlight.and_then(|h| {
						if self.current_start < offset {
							Some(HighlightSpan {
								start: self.current_start,
								end: offset,
								highlight: h,
							})
						} else {
							None
						}
					});
					self.current_start = offset;
					self.current_highlight = new_highlight;
					span
				}
			};

			if span.is_some() {
				return span;
			}
		}

		// Emit final span if any
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
	use ratatui::style::Color;

	use super::*;

	#[test]
	fn test_highlight_styles() {
		let scopes = vec!["keyword".to_string(), "string".to_string()];

		let styles = HighlightStyles::new(scopes, |scope| match scope {
			"keyword" => Style::default().fg(Color::Red),
			"string" => Style::default().fg(Color::Green),
			_ => Style::default(),
		});

		assert_eq!(styles.scopes().len(), 2);
		assert_eq!(
			styles.style_for_highlight(Highlight::new(0)),
			Style::default().fg(Color::Red)
		);
		assert_eq!(
			styles.style_for_highlight(Highlight::new(1)),
			Style::default().fg(Color::Green)
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
