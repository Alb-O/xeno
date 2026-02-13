//! Fill span generation for empty space in lines.
//!
//! Provides utilities for creating fill spans that extend line backgrounds
//! to the full terminal width.

use xeno_primitives::{Color, Style};

use crate::render::RenderSpan;

/// Configuration for filling empty space in a line.
#[derive(Debug, Clone, Copy)]
pub struct FillConfig {
	/// Background color for the fill, if any.
	pub bg: Option<Color>,
}

impl FillConfig {
	/// Creates a fill config from an optional background color.
	pub const fn from_bg(bg: Option<Color>) -> Self {
		Self { bg }
	}

	/// Creates a fill span of the given width, or `None` if no background.
	pub fn fill_span(self, width: usize) -> Option<RenderSpan<'static>> {
		self.bg.map(|bg| RenderSpan::styled(" ".repeat(width), Style::default().bg(bg)))
	}
}

impl From<Option<Color>> for FillConfig {
	fn from(bg: Option<Color>) -> Self {
		Self::from_bg(bg)
	}
}

#[cfg(test)]
mod tests;
