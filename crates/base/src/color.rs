//! Abstract color and style types for theming.
//!
//! These types define colors and text modifiers without depending on any
//! terminal or UI library. Conversion to ratatui/crossterm types happens
//! at the UI boundary in tome-theme or tome-ui.

use serde::{Deserialize, Serialize};

/// An abstract color representation.
///
/// This enum mirrors standard ANSI terminal color capabilities.
/// Colors 0-7 are the regular colors, 8-15 are bright variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Color {
	/// Resets the terminal color to default.
	#[default]
	Reset,
	/// ANSI 0: Black
	Black,
	/// ANSI 1: Red
	Red,
	/// ANSI 2: Green
	Green,
	/// ANSI 3: Yellow
	Yellow,
	/// ANSI 4: Blue
	Blue,
	/// ANSI 5: Magenta
	Magenta,
	/// ANSI 6: Cyan
	Cyan,
	/// ANSI 7: White (often rendered as light gray)
	Gray,
	/// ANSI 8: Bright Black (dark gray)
	DarkGray,
	/// ANSI 9: Bright Red
	LightRed,
	/// ANSI 10: Bright Green
	LightGreen,
	/// ANSI 11: Bright Yellow
	LightYellow,
	/// ANSI 12: Bright Blue
	LightBlue,
	/// ANSI 13: Bright Magenta
	LightMagenta,
	/// ANSI 14: Bright Cyan
	LightCyan,
	/// ANSI 15: Bright White
	White,
	/// True color RGB.
	Rgb(u8, u8, u8),
	/// 256-color palette index.
	Indexed(u8),
}

/// Text style modifiers (bold, italic, underline, etc.).
///
/// This is a bitflags-style struct for combining multiple modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Modifier(u16);

impl Modifier {
	pub const NONE: Self = Self(0);
	pub const BOLD: Self = Self(1 << 0);
	pub const DIM: Self = Self(1 << 1);
	pub const ITALIC: Self = Self(1 << 2);
	pub const UNDERLINED: Self = Self(1 << 3);
	pub const SLOW_BLINK: Self = Self(1 << 4);
	pub const RAPID_BLINK: Self = Self(1 << 5);
	pub const REVERSED: Self = Self(1 << 6);
	pub const HIDDEN: Self = Self(1 << 7);
	pub const CROSSED_OUT: Self = Self(1 << 8);

	/// Creates an empty modifier set.
	#[inline]
	pub const fn empty() -> Self {
		Self(0)
	}

	/// Returns true if no modifiers are set.
	#[inline]
	pub const fn is_empty(self) -> bool {
		self.0 == 0
	}

	/// Returns true if the modifier contains the given modifier.
	#[inline]
	pub const fn contains(self, other: Self) -> bool {
		(self.0 & other.0) == other.0
	}

	/// Combines two modifiers.
	#[inline]
	pub const fn union(self, other: Self) -> Self {
		Self(self.0 | other.0)
	}

	/// Returns the raw bits for conversion.
	#[inline]
	pub const fn bits(self) -> u16 {
		self.0
	}

	/// Creates a modifier from raw bits.
	#[inline]
	pub const fn from_bits(bits: u16) -> Self {
		Self(bits)
	}
}

impl std::ops::BitOr for Modifier {
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self::Output {
		self.union(rhs)
	}
}

impl std::ops::BitOrAssign for Modifier {
	fn bitor_assign(&mut self, rhs: Self) {
		*self = self.union(rhs);
	}
}

/// A complete text style with optional foreground, background, and modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
	pub fg: Option<Color>,
	pub bg: Option<Color>,
	pub modifiers: Modifier,
}

impl Style {
	/// Creates an empty style.
	pub const fn new() -> Self {
		Self {
			fg: None,
			bg: None,
			modifiers: Modifier::NONE,
		}
	}

	/// Sets the foreground color.
	pub const fn fg(mut self, color: Color) -> Self {
		self.fg = Some(color);
		self
	}

	/// Sets the background color.
	pub const fn bg(mut self, color: Color) -> Self {
		self.bg = Some(color);
		self
	}

	/// Adds modifiers.
	pub const fn add_modifier(mut self, modifier: Modifier) -> Self {
		self.modifiers = self.modifiers.union(modifier);
		self
	}
}

// Conversion to ratatui types - implemented via From traits
// These allow downstream crates with ratatui to convert easily

#[cfg(feature = "ratatui")]
impl From<Color> for ratatui::style::Color {
	fn from(color: Color) -> Self {
		match color {
			Color::Reset => ratatui::style::Color::Reset,
			Color::Black => ratatui::style::Color::Black,
			Color::Red => ratatui::style::Color::Red,
			Color::Green => ratatui::style::Color::Green,
			Color::Yellow => ratatui::style::Color::Yellow,
			Color::Blue => ratatui::style::Color::Blue,
			Color::Magenta => ratatui::style::Color::Magenta,
			Color::Cyan => ratatui::style::Color::Cyan,
			Color::Gray => ratatui::style::Color::Gray,
			Color::DarkGray => ratatui::style::Color::DarkGray,
			Color::LightRed => ratatui::style::Color::LightRed,
			Color::LightGreen => ratatui::style::Color::LightGreen,
			Color::LightYellow => ratatui::style::Color::LightYellow,
			Color::LightBlue => ratatui::style::Color::LightBlue,
			Color::LightMagenta => ratatui::style::Color::LightMagenta,
			Color::LightCyan => ratatui::style::Color::LightCyan,
			Color::White => ratatui::style::Color::White,
			Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
			Color::Indexed(i) => ratatui::style::Color::Indexed(i),
		}
	}
}

#[cfg(feature = "ratatui")]
impl From<ratatui::style::Color> for Color {
	fn from(color: ratatui::style::Color) -> Self {
		match color {
			ratatui::style::Color::Reset => Color::Reset,
			ratatui::style::Color::Black => Color::Black,
			ratatui::style::Color::Red => Color::Red,
			ratatui::style::Color::Green => Color::Green,
			ratatui::style::Color::Yellow => Color::Yellow,
			ratatui::style::Color::Blue => Color::Blue,
			ratatui::style::Color::Magenta => Color::Magenta,
			ratatui::style::Color::Cyan => Color::Cyan,
			ratatui::style::Color::Gray => Color::Gray,
			ratatui::style::Color::DarkGray => Color::DarkGray,
			ratatui::style::Color::LightRed => Color::LightRed,
			ratatui::style::Color::LightGreen => Color::LightGreen,
			ratatui::style::Color::LightYellow => Color::LightYellow,
			ratatui::style::Color::LightBlue => Color::LightBlue,
			ratatui::style::Color::LightMagenta => Color::LightMagenta,
			ratatui::style::Color::LightCyan => Color::LightCyan,
			ratatui::style::Color::White => Color::White,
			ratatui::style::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
			ratatui::style::Color::Indexed(i) => Color::Indexed(i),
		}
	}
}

#[cfg(feature = "ratatui")]
impl From<Modifier> for ratatui::style::Modifier {
	fn from(m: Modifier) -> Self {
		let mut result = ratatui::style::Modifier::empty();
		if m.contains(Modifier::BOLD) {
			result |= ratatui::style::Modifier::BOLD;
		}
		if m.contains(Modifier::DIM) {
			result |= ratatui::style::Modifier::DIM;
		}
		if m.contains(Modifier::ITALIC) {
			result |= ratatui::style::Modifier::ITALIC;
		}
		if m.contains(Modifier::UNDERLINED) {
			result |= ratatui::style::Modifier::UNDERLINED;
		}
		if m.contains(Modifier::SLOW_BLINK) {
			result |= ratatui::style::Modifier::SLOW_BLINK;
		}
		if m.contains(Modifier::RAPID_BLINK) {
			result |= ratatui::style::Modifier::RAPID_BLINK;
		}
		if m.contains(Modifier::REVERSED) {
			result |= ratatui::style::Modifier::REVERSED;
		}
		if m.contains(Modifier::HIDDEN) {
			result |= ratatui::style::Modifier::HIDDEN;
		}
		if m.contains(Modifier::CROSSED_OUT) {
			result |= ratatui::style::Modifier::CROSSED_OUT;
		}
		result
	}
}

#[cfg(feature = "ratatui")]
impl From<ratatui::style::Modifier> for Modifier {
	fn from(m: ratatui::style::Modifier) -> Self {
		let mut result = Modifier::NONE;
		if m.contains(ratatui::style::Modifier::BOLD) {
			result |= Modifier::BOLD;
		}
		if m.contains(ratatui::style::Modifier::DIM) {
			result |= Modifier::DIM;
		}
		if m.contains(ratatui::style::Modifier::ITALIC) {
			result |= Modifier::ITALIC;
		}
		if m.contains(ratatui::style::Modifier::UNDERLINED) {
			result |= Modifier::UNDERLINED;
		}
		if m.contains(ratatui::style::Modifier::SLOW_BLINK) {
			result |= Modifier::SLOW_BLINK;
		}
		if m.contains(ratatui::style::Modifier::RAPID_BLINK) {
			result |= Modifier::RAPID_BLINK;
		}
		if m.contains(ratatui::style::Modifier::REVERSED) {
			result |= Modifier::REVERSED;
		}
		if m.contains(ratatui::style::Modifier::HIDDEN) {
			result |= Modifier::HIDDEN;
		}
		if m.contains(ratatui::style::Modifier::CROSSED_OUT) {
			result |= Modifier::CROSSED_OUT;
		}
		result
	}
}

#[cfg(feature = "ratatui")]
impl From<Style> for ratatui::style::Style {
	fn from(style: Style) -> Self {
		let mut result = ratatui::style::Style::default();
		if let Some(fg) = style.fg {
			result = result.fg(fg.into());
		}
		if let Some(bg) = style.bg {
			result = result.bg(bg.into());
		}
		if !style.modifiers.is_empty() {
			result = result.add_modifier(style.modifiers.into());
		}
		result
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_modifier_combine() {
		let bold_italic = Modifier::BOLD | Modifier::ITALIC;
		assert!(bold_italic.contains(Modifier::BOLD));
		assert!(bold_italic.contains(Modifier::ITALIC));
		assert!(!bold_italic.contains(Modifier::UNDERLINED));
	}

	#[test]
	fn test_style_builder() {
		let style = Style::new()
			.fg(Color::Red)
			.bg(Color::Black)
			.add_modifier(Modifier::BOLD);

		assert_eq!(style.fg, Some(Color::Red));
		assert_eq!(style.bg, Some(Color::Black));
		assert!(style.modifiers.contains(Modifier::BOLD));
	}

	#[cfg(feature = "ratatui")]
	#[test]
	fn test_color_conversion_roundtrip() {
		let color = Color::Rgb(128, 64, 255);
		let ratatui_color: ratatui::style::Color = color.into();
		let back: Color = ratatui_color.into();
		assert_eq!(color, back);
	}
}
