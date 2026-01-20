//! Fill span generation for empty space in lines.
//!
//! Provides utilities for creating fill spans that extend line backgrounds
//! to the full terminal width.

use xeno_tui::style::{Color, Style};
use xeno_tui::text::Span;

/// Configuration for filling empty space in a line.
#[derive(Debug, Clone, Copy)]
pub struct FillConfig {
	/// Background color for the fill, if any.
	pub bg: Option<Color>,
}

impl FillConfig {
	/// Creates a fill config with no background.
	#[allow(dead_code, reason = "utility constructor for callers")]
	pub const fn none() -> Self {
		Self { bg: None }
	}

	/// Creates a fill config with the given background color.
	#[allow(dead_code, reason = "utility constructor for callers")]
	pub const fn with_bg(bg: Color) -> Self {
		Self { bg: Some(bg) }
	}

	/// Creates a fill config from an optional background color.
	pub const fn from_bg(bg: Option<Color>) -> Self {
		Self { bg }
	}

	/// Returns whether this fill has a background.
	#[allow(dead_code, reason = "utility method for callers")]
	pub fn has_bg(&self) -> bool {
		self.bg.is_some()
	}

	/// Creates a fill span of the given width, or `None` if no background.
	pub fn fill_span(self, width: usize) -> Option<Span<'static>> {
		self.bg
			.map(|bg| Span::styled(" ".repeat(width), Style::default().bg(bg)))
	}

	/// Creates a fill span, returning an empty span if no background.
	#[allow(dead_code, reason = "utility method for callers")]
	pub fn fill_span_or_empty(self, width: usize) -> Span<'static> {
		self.fill_span(width)
			.unwrap_or_else(|| Span::raw(String::new()))
	}
}

impl From<Option<Color>> for FillConfig {
	fn from(bg: Option<Color>) -> Self {
		Self::from_bg(bg)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fill_none() {
		let fill = FillConfig::none();
		assert!(!fill.has_bg());
		assert!(fill.fill_span(10).is_none());
	}

	#[test]
	fn fill_with_bg() {
		let fill = FillConfig::with_bg(Color::Rgb(50, 50, 50));
		assert!(fill.has_bg());
		let span = fill.fill_span(5).unwrap();
		assert_eq!(span.content.len(), 5);
	}

	#[test]
	fn fill_from_option() {
		let fill: FillConfig = Some(Color::Red).into();
		assert!(fill.has_bg());

		let fill: FillConfig = None.into();
		assert!(!fill.has_bg());
	}
}
