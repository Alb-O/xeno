//! Abstract color and style types for theming.
//!
//! These types define colors and text modifiers without depending on any
//! terminal or UI library. Conversion to tome_tui/crossterm types happens
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

// Conversion to tome_tui types - implemented via From traits
// These allow downstream crates with tome_tui to convert easily

#[cfg(feature = "tome-tui")]
impl From<Color> for tome_tui::style::Color {
	fn from(color: Color) -> Self {
		match color {
			Color::Reset => tome_tui::style::Color::Reset,
			Color::Black => tome_tui::style::Color::Black,
			Color::Red => tome_tui::style::Color::Red,
			Color::Green => tome_tui::style::Color::Green,
			Color::Yellow => tome_tui::style::Color::Yellow,
			Color::Blue => tome_tui::style::Color::Blue,
			Color::Magenta => tome_tui::style::Color::Magenta,
			Color::Cyan => tome_tui::style::Color::Cyan,
			Color::Gray => tome_tui::style::Color::Gray,
			Color::DarkGray => tome_tui::style::Color::DarkGray,
			Color::LightRed => tome_tui::style::Color::LightRed,
			Color::LightGreen => tome_tui::style::Color::LightGreen,
			Color::LightYellow => tome_tui::style::Color::LightYellow,
			Color::LightBlue => tome_tui::style::Color::LightBlue,
			Color::LightMagenta => tome_tui::style::Color::LightMagenta,
			Color::LightCyan => tome_tui::style::Color::LightCyan,
			Color::White => tome_tui::style::Color::White,
			Color::Rgb(r, g, b) => tome_tui::style::Color::Rgb(r, g, b),
			Color::Indexed(i) => tome_tui::style::Color::Indexed(i),
		}
	}
}

#[cfg(feature = "tome-tui")]
impl From<tome_tui::style::Color> for Color {
	fn from(color: tome_tui::style::Color) -> Self {
		match color {
			tome_tui::style::Color::Reset => Color::Reset,
			tome_tui::style::Color::Black => Color::Black,
			tome_tui::style::Color::Red => Color::Red,
			tome_tui::style::Color::Green => Color::Green,
			tome_tui::style::Color::Yellow => Color::Yellow,
			tome_tui::style::Color::Blue => Color::Blue,
			tome_tui::style::Color::Magenta => Color::Magenta,
			tome_tui::style::Color::Cyan => Color::Cyan,
			tome_tui::style::Color::Gray => Color::Gray,
			tome_tui::style::Color::DarkGray => Color::DarkGray,
			tome_tui::style::Color::LightRed => Color::LightRed,
			tome_tui::style::Color::LightGreen => Color::LightGreen,
			tome_tui::style::Color::LightYellow => Color::LightYellow,
			tome_tui::style::Color::LightBlue => Color::LightBlue,
			tome_tui::style::Color::LightMagenta => Color::LightMagenta,
			tome_tui::style::Color::LightCyan => Color::LightCyan,
			tome_tui::style::Color::White => Color::White,
			tome_tui::style::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
			tome_tui::style::Color::Indexed(i) => Color::Indexed(i),
		}
	}
}

#[cfg(feature = "tome-tui")]
impl From<Modifier> for tome_tui::style::Modifier {
	fn from(m: Modifier) -> Self {
		let mut result = tome_tui::style::Modifier::empty();
		if m.contains(Modifier::BOLD) {
			result |= tome_tui::style::Modifier::BOLD;
		}
		if m.contains(Modifier::DIM) {
			result |= tome_tui::style::Modifier::DIM;
		}
		if m.contains(Modifier::ITALIC) {
			result |= tome_tui::style::Modifier::ITALIC;
		}
		if m.contains(Modifier::UNDERLINED) {
			result |= tome_tui::style::Modifier::UNDERLINED;
		}
		if m.contains(Modifier::SLOW_BLINK) {
			result |= tome_tui::style::Modifier::SLOW_BLINK;
		}
		if m.contains(Modifier::RAPID_BLINK) {
			result |= tome_tui::style::Modifier::RAPID_BLINK;
		}
		if m.contains(Modifier::REVERSED) {
			result |= tome_tui::style::Modifier::REVERSED;
		}
		if m.contains(Modifier::HIDDEN) {
			result |= tome_tui::style::Modifier::HIDDEN;
		}
		if m.contains(Modifier::CROSSED_OUT) {
			result |= tome_tui::style::Modifier::CROSSED_OUT;
		}
		result
	}
}

#[cfg(feature = "tome-tui")]
impl From<tome_tui::style::Modifier> for Modifier {
	fn from(m: tome_tui::style::Modifier) -> Self {
		let mut result = Modifier::NONE;
		if m.contains(tome_tui::style::Modifier::BOLD) {
			result |= Modifier::BOLD;
		}
		if m.contains(tome_tui::style::Modifier::DIM) {
			result |= Modifier::DIM;
		}
		if m.contains(tome_tui::style::Modifier::ITALIC) {
			result |= Modifier::ITALIC;
		}
		if m.contains(tome_tui::style::Modifier::UNDERLINED) {
			result |= Modifier::UNDERLINED;
		}
		if m.contains(tome_tui::style::Modifier::SLOW_BLINK) {
			result |= Modifier::SLOW_BLINK;
		}
		if m.contains(tome_tui::style::Modifier::RAPID_BLINK) {
			result |= Modifier::RAPID_BLINK;
		}
		if m.contains(tome_tui::style::Modifier::REVERSED) {
			result |= Modifier::REVERSED;
		}
		if m.contains(tome_tui::style::Modifier::HIDDEN) {
			result |= Modifier::HIDDEN;
		}
		if m.contains(tome_tui::style::Modifier::CROSSED_OUT) {
			result |= Modifier::CROSSED_OUT;
		}
		result
	}
}

#[cfg(feature = "tome-tui")]
impl From<Style> for tome_tui::style::Style {
	fn from(style: Style) -> Self {
		let mut result = tome_tui::style::Style::default();
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

impl Color {
	/// Converts the color to RGB values if possible.
	///
	/// Returns approximate RGB values for ANSI colors.
	pub const fn to_rgb(self) -> Option<(u8, u8, u8)> {
		match self {
			Color::Black => Some((0, 0, 0)),
			Color::Red => Some((205, 49, 49)),
			Color::Green => Some((13, 188, 121)),
			Color::Yellow => Some((229, 229, 16)),
			Color::Blue => Some((36, 114, 200)),
			Color::Magenta => Some((188, 63, 188)),
			Color::Cyan => Some((17, 168, 205)),
			Color::Gray => Some((128, 128, 128)),
			Color::DarkGray => Some((102, 102, 102)),
			Color::LightRed => Some((241, 76, 76)),
			Color::LightGreen => Some((35, 209, 139)),
			Color::LightYellow => Some((245, 245, 67)),
			Color::LightBlue => Some((59, 142, 234)),
			Color::LightMagenta => Some((214, 112, 214)),
			Color::LightCyan => Some((41, 184, 219)),
			Color::White => Some((229, 229, 229)),
			Color::Rgb(r, g, b) => Some((r, g, b)),
			// Indexed colors would need a palette lookup
			Color::Indexed(_) | Color::Reset => None,
		}
	}

	/// Linearly interpolate between two colors.
	///
	/// `t=0.0` returns `self`, `t=1.0` returns `target`.
	/// For RGB colors, performs component-wise interpolation.
	/// For ANSI colors, converts to RGB first.
	/// For non-interpolatable colors (Indexed, Reset), snaps at midpoint.
	pub fn lerp(self, target: Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		match (self.to_rgb(), target.to_rgb()) {
			(Some((r1, g1, b1)), Some((r2, g2, b2))) => {
				let lerp_u8 =
					|a: u8, b: u8| -> u8 { (a as f32 + (b as f32 - a as f32) * t).round() as u8 };
				Color::Rgb(lerp_u8(r1, r2), lerp_u8(g1, g2), lerp_u8(b1, b2))
			}
			_ => {
				if t > 0.5 {
					target
				} else {
					self
				}
			}
		}
	}

	/// Blend this color with another using alpha.
	///
	/// `alpha=0.0` returns `other`, `alpha=1.0` returns `self`.
	/// This is equivalent to `other.lerp(self, alpha)`.
	#[inline]
	pub fn blend(self, other: Self, alpha: f32) -> Self {
		other.lerp(self, alpha)
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

	#[cfg(feature = "tome-tui")]
	#[test]
	fn test_color_conversion_roundtrip() {
		let color = Color::Rgb(128, 64, 255);
		let tome_tui_color: tome_tui::style::Color = color.into();
		let back: Color = tome_tui_color.into();
		assert_eq!(color, back);
	}

	#[test]
	fn test_to_rgb() {
		assert_eq!(Color::Black.to_rgb(), Some((0, 0, 0)));
		assert_eq!(Color::Rgb(100, 150, 200).to_rgb(), Some((100, 150, 200)));
		assert_eq!(Color::Indexed(42).to_rgb(), None);
		assert_eq!(Color::Reset.to_rgb(), None);
	}

	#[test]
	fn test_color_lerp_rgb() {
		let black = Color::Rgb(0, 0, 0);
		let white = Color::Rgb(255, 255, 255);

		assert_eq!(black.lerp(white, 0.0), Color::Rgb(0, 0, 0));
		assert_eq!(black.lerp(white, 0.5), Color::Rgb(128, 128, 128));
		assert_eq!(black.lerp(white, 1.0), Color::Rgb(255, 255, 255));
	}

	#[test]
	fn test_color_lerp_ansi() {
		// ANSI colors are converted to RGB for lerping
		let black = Color::Black;
		let white = Color::White;
		let mid = black.lerp(white, 0.5);

		// Result should be RGB (midpoint between approximate RGB values)
		match mid {
			Color::Rgb(_, _, _) => {}
			_ => panic!("Expected RGB color from lerping ANSI colors"),
		}
	}

	#[test]
	fn test_color_lerp_clamps() {
		let a = Color::Rgb(0, 0, 0);
		let b = Color::Rgb(100, 100, 100);

		// t < 0 should clamp to 0
		assert_eq!(a.lerp(b, -0.5), Color::Rgb(0, 0, 0));
		// t > 1 should clamp to 1
		assert_eq!(a.lerp(b, 1.5), Color::Rgb(100, 100, 100));
	}

	#[test]
	fn test_color_blend() {
		let fg = Color::Rgb(255, 255, 255);
		let bg = Color::Rgb(0, 0, 0);

		// alpha=1.0 returns fg
		assert_eq!(fg.blend(bg, 1.0), Color::Rgb(255, 255, 255));
		// alpha=0.0 returns bg
		assert_eq!(fg.blend(bg, 0.0), Color::Rgb(0, 0, 0));
		// alpha=0.5 returns midpoint
		assert_eq!(fg.blend(bg, 0.5), Color::Rgb(128, 128, 128));
	}
}
