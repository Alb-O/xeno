//! Icon widget for rendering nerd font icons.
//!
//! This module provides a simple widget for rendering single-character icons
//! (typically nerd font glyphs) with styling support.

use alloc::borrow::Cow;

use unicode_width::UnicodeWidthStr;

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Style, Styled};
use crate::widgets::Widget;

/// Preset icons for common semantic types.
///
/// These use nerd font glyphs from the Font Awesome set (nf-fa-*),
/// which have broad support across nerd font patched fonts.
pub mod presets {
	/// Info icon (nf-fa-info_circle, U+F05A).
	pub const INFO: &str = "\u{F05A}";
	/// Warning icon (nf-fa-exclamation_triangle, U+F071).
	pub const WARNING: &str = "\u{F071}";
	/// Error icon (nf-fa-times_circle, U+F057).
	pub const ERROR: &str = "\u{F057}";
	/// Success/check icon (nf-fa-check_circle, U+F058).
	pub const SUCCESS: &str = "\u{F058}";
	/// Debug icon (nf-fa-bug, U+F188).
	pub const DEBUG: &str = "\u{F188}";
	/// Trace icon (nf-fa-ellipsis_h, U+F141).
	pub const TRACE: &str = "\u{F141}";
}

/// A widget for rendering a single icon with optional styling.
///
/// Icons are typically nerd font glyphs that occupy 1-2 terminal cells.
/// This widget renders the icon at the top-left of the given area.
///
/// # Example
///
/// ```ignore
/// use tome_tui::widgets::{Icon, icon::presets};
/// use tome_tui::style::{Style, Color};
///
/// let icon = Icon::new(presets::INFO)
///     .style(Style::default().fg(Color::Blue));
///
/// frame.render_widget(icon, area);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon<'a> {
	/// The icon glyph to render.
	glyph: Cow<'a, str>,
	/// Style to apply to the icon.
	style: Style,
}

impl<'a> Icon<'a> {
	/// Creates a new icon with the given glyph.
	pub fn new(glyph: impl Into<Cow<'a, str>>) -> Self {
		Self {
			glyph: glyph.into(),
			style: Style::default(),
		}
	}

	/// Sets the style of the icon.
	#[must_use]
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Returns the display width of the icon in terminal cells.
	pub fn width(&self) -> usize {
		self.glyph.width()
	}

	/// Returns the icon glyph.
	pub fn glyph(&self) -> &str {
		&self.glyph
	}
}

impl Default for Icon<'_> {
	fn default() -> Self {
		Self {
			glyph: Cow::Borrowed(""),
			style: Style::default(),
		}
	}
}

impl<'a> From<&'a str> for Icon<'a> {
	fn from(glyph: &'a str) -> Self {
		Self::new(glyph)
	}
}

impl Styled for Icon<'_> {
	type Item = Self;

	fn style(&self) -> Style {
		self.style
	}

	fn set_style<S: Into<Style>>(mut self, style: S) -> Self::Item {
		self.style = style.into();
		self
	}
}

impl Widget for Icon<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		Widget::render(&self, area, buf);
	}
}

impl Widget for &Icon<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		if area.is_empty() || self.glyph.is_empty() {
			return;
		}

		// Render the icon at the top-left of the area
		buf.set_string(area.x, area.y, &self.glyph, self.style);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::style::Color;

	#[test]
	fn new_icon() {
		let icon = Icon::new(presets::INFO);
		assert_eq!(icon.glyph(), presets::INFO);
	}

	#[test]
	fn icon_width() {
		let icon = Icon::new(presets::INFO);
		// Nerd font icons are typically 1-2 cells wide
		assert!(icon.width() >= 1);
	}

	#[test]
	fn icon_with_style() {
		let style = Style::default().fg(Color::Red);
		let icon = Icon::new(presets::ERROR).style(style);
		assert_eq!(icon.style, style);
	}

	#[test]
	fn render_icon() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
		let icon = Icon::new("X").style(Style::default().fg(Color::Red));
		icon.render(buf.area, &mut buf);

		assert_eq!(buf[(0, 0)].symbol(), "X");
		assert_eq!(buf[(0, 0)].fg, Color::Red);
	}

	#[test]
	fn render_empty_area() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
		let icon = Icon::new(presets::INFO);
		icon.render(Rect::ZERO, &mut buf);
		// Should not panic, buffer unchanged
	}
}
