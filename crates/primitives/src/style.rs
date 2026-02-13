//! Backend-neutral style primitives shared across frontends.
//!
//! These types intentionally mirror the subset of style semantics needed by
//! editor core/render policy while remaining independent from any frontend UI
//! toolkit implementation.

use core::fmt;
use core::str::FromStr;

use bitflags::bitflags;

/// Backend-neutral color model.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Color {
	/// Reset to backend default.
	#[default]
	Reset,
	Black,
	Red,
	Green,
	Yellow,
	Blue,
	Magenta,
	Cyan,
	Gray,
	DarkGray,
	LightRed,
	LightGreen,
	LightYellow,
	LightBlue,
	LightMagenta,
	LightCyan,
	White,
	Rgb(u8, u8, u8),
	Indexed(u8),
}

/// Error returned when parsing a color from string fails.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ParseColorError;

impl fmt::Display for ParseColorError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("failed to parse color")
	}
}

impl core::error::Error for ParseColorError {}

impl Color {
	/// Constructs a color from a packed `0xRRGGBB` value.
	pub const fn from_u32(value: u32) -> Self {
		let r = ((value >> 16) & 0xff) as u8;
		let g = ((value >> 8) & 0xff) as u8;
		let b = (value & 0xff) as u8;
		Self::Rgb(r, g, b)
	}

	/// Converts the color to RGB components.
	pub fn to_rgb(self) -> (u8, u8, u8) {
		match self {
			Color::Reset => (0, 0, 0),
			Color::Black => (0x00, 0x00, 0x00),
			Color::Red => (0x80, 0x00, 0x00),
			Color::Green => (0x00, 0x80, 0x00),
			Color::Yellow => (0x80, 0x80, 0x00),
			Color::Blue => (0x00, 0x00, 0x80),
			Color::Magenta => (0x80, 0x00, 0x80),
			Color::Cyan => (0x00, 0x80, 0x80),
			Color::Gray => (0xc0, 0xc0, 0xc0),
			Color::DarkGray => (0x80, 0x80, 0x80),
			Color::LightRed => (0xff, 0x00, 0x00),
			Color::LightGreen => (0x00, 0xff, 0x00),
			Color::LightYellow => (0xff, 0xff, 0x00),
			Color::LightBlue => (0x00, 0x00, 0xff),
			Color::LightMagenta => (0xff, 0x00, 0xff),
			Color::LightCyan => (0x00, 0xff, 0xff),
			Color::White => (0xff, 0xff, 0xff),
			Color::Rgb(r, g, b) => (r, g, b),
			Color::Indexed(index) => index_to_rgb(index),
		}
	}

	/// Blends this color with another using alpha compositing.
	///
	/// `alpha=0.0` yields `other`, `alpha=1.0` yields `self`.
	pub fn blend(self, other: Self, alpha: f32) -> Self {
		let (r1, g1, b1) = self.to_rgb();
		let (r2, g2, b2) = other.to_rgb();
		let alpha = alpha.clamp(0.0, 1.0);
		let blend = |a: u8, b: u8| (a as f32 * alpha + b as f32 * (1.0 - alpha)).round() as u8;
		Self::Rgb(blend(r1, r2), blend(g1, g2), blend(b1, b2))
	}

	/// Computes relative luminance per WCAG.
	pub fn luminance(self) -> f32 {
		let (r, g, b) = self.to_rgb();
		let to_linear = |c: u8| {
			let c = c as f32 / 255.0;
			if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
		};
		0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
	}

	/// Computes WCAG contrast ratio against `other`.
	pub fn contrast_ratio(self, other: Self) -> f32 {
		let l1 = self.luminance();
		let l2 = other.luminance();
		let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
		(lighter + 0.05) / (darker + 0.05)
	}

	/// Ensures minimum contrast against a background color.
	pub fn ensure_min_contrast(self, background: Self, min_ratio: f32) -> Self {
		if self.contrast_ratio(background) >= min_ratio {
			return self;
		}

		let bg_lum = background.luminance();
		let target = if bg_lum > 0.5 { Self::Black } else { Self::White };

		let mut low = 0.0_f32;
		let mut high = 1.0_f32;
		for _ in 0..8 {
			let mid = (low + high) / 2.0;
			let candidate = self.blend(target, 1.0 - mid);
			if candidate.contrast_ratio(background) >= min_ratio {
				high = mid;
			} else {
				low = mid;
			}
		}
		self.blend(target, 1.0 - high)
	}
}

fn index_to_rgb(index: u8) -> (u8, u8, u8) {
	const BASE: [(u8, u8, u8); 16] = [
		(0x00, 0x00, 0x00),
		(0x80, 0x00, 0x00),
		(0x00, 0x80, 0x00),
		(0x80, 0x80, 0x00),
		(0x00, 0x00, 0x80),
		(0x80, 0x00, 0x80),
		(0x00, 0x80, 0x80),
		(0xc0, 0xc0, 0xc0),
		(0x80, 0x80, 0x80),
		(0xff, 0x00, 0x00),
		(0x00, 0xff, 0x00),
		(0xff, 0xff, 0x00),
		(0x00, 0x00, 0xff),
		(0xff, 0x00, 0xff),
		(0x00, 0xff, 0xff),
		(0xff, 0xff, 0xff),
	];
	const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

	if index < 16 {
		return BASE[index as usize];
	}
	if (16..=231).contains(&index) {
		let value = index - 16;
		let r = CUBE[(value / 36) as usize];
		let g = CUBE[((value % 36) / 6) as usize];
		let b = CUBE[(value % 6) as usize];
		return (r, g, b);
	}
	let gray = 8u8.saturating_add((index - 232) * 10);
	(gray, gray, gray)
}

impl FromStr for Color {
	type Err = ParseColorError;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		let normalized = input
			.trim()
			.to_lowercase()
			.replace([' ', '-', '_'], "")
			.replace("bright", "light")
			.replace("grey", "gray")
			.replace("silver", "gray")
			.replace("lightblack", "darkgray")
			.replace("lightwhite", "white")
			.replace("lightgray", "white");

		let parsed = match normalized.as_str() {
			"reset" | "default" => Color::Reset,
			"black" => Color::Black,
			"red" => Color::Red,
			"green" => Color::Green,
			"yellow" => Color::Yellow,
			"blue" => Color::Blue,
			"magenta" => Color::Magenta,
			"cyan" => Color::Cyan,
			"gray" => Color::Gray,
			"darkgray" => Color::DarkGray,
			"lightred" => Color::LightRed,
			"lightgreen" => Color::LightGreen,
			"lightyellow" => Color::LightYellow,
			"lightblue" => Color::LightBlue,
			"lightmagenta" => Color::LightMagenta,
			"lightcyan" => Color::LightCyan,
			"white" => Color::White,
			_ => {
				if let Some(hex) = normalized.strip_prefix('#')
					&& hex.len() == 6
				{
					let value = u32::from_str_radix(hex, 16).map_err(|_| ParseColorError)?;
					return Ok(Color::from_u32(value));
				}
				if let Ok(index) = normalized.parse::<u8>() {
					return Ok(Color::Indexed(index));
				}
				return Err(ParseColorError);
			}
		};
		Ok(parsed)
	}
}

bitflags! {
	/// Text style modifiers.
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct Modifier: u16 {
		const BOLD = 0b0000_0001;
		const DIM = 0b0000_0010;
		const ITALIC = 0b0000_0100;
		const UNDERLINED = 0b0000_1000;
		const SLOW_BLINK = 0b0001_0000;
		const RAPID_BLINK = 0b0010_0000;
		const REVERSED = 0b0100_0000;
		const HIDDEN = 0b1000_0000;
		const CROSSED_OUT = 0b0001_0000_0000;
	}
}

impl Default for Modifier {
	fn default() -> Self {
		Self::empty()
	}
}

/// Underline rendering mode.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UnderlineStyle {
	/// Reset underline style to backend default.
	#[default]
	Reset,
	Line,
	Curl,
	Dotted,
	Dashed,
	DoubleLine,
}

/// Incremental style patch.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Style {
	pub fg: Option<Color>,
	pub bg: Option<Color>,
	pub underline_color: Option<Color>,
	pub underline_style: Option<UnderlineStyle>,
	pub add_modifier: Modifier,
	pub sub_modifier: Modifier,
}

impl Style {
	pub const fn new() -> Self {
		Self {
			fg: None,
			bg: None,
			underline_color: None,
			underline_style: None,
			add_modifier: Modifier::empty(),
			sub_modifier: Modifier::empty(),
		}
	}

	pub const fn reset() -> Self {
		Self {
			fg: Some(Color::Reset),
			bg: Some(Color::Reset),
			underline_color: Some(Color::Reset),
			underline_style: Some(UnderlineStyle::Reset),
			add_modifier: Modifier::empty(),
			sub_modifier: Modifier::all(),
		}
	}

	pub const fn fg(mut self, color: Color) -> Self {
		self.fg = Some(color);
		self
	}

	pub const fn bg(mut self, color: Color) -> Self {
		self.bg = Some(color);
		self
	}

	pub const fn underline_color(mut self, color: Color) -> Self {
		self.underline_color = Some(color);
		self
	}

	pub const fn underline_style(mut self, style: UnderlineStyle) -> Self {
		self.underline_style = Some(style);
		self
	}

	pub const fn add_modifier(mut self, modifier: Modifier) -> Self {
		self.sub_modifier = self.sub_modifier.difference(modifier);
		self.add_modifier = self.add_modifier.union(modifier);
		self
	}

	pub const fn remove_modifier(mut self, modifier: Modifier) -> Self {
		self.add_modifier = self.add_modifier.difference(modifier);
		self.sub_modifier = self.sub_modifier.union(modifier);
		self
	}

	pub const fn has_modifier(self, modifier: Modifier) -> bool {
		self.add_modifier.contains(modifier) && !self.sub_modifier.contains(modifier)
	}

	pub fn patch<S: Into<Self>>(mut self, other: S) -> Self {
		let other = other.into();
		self.fg = other.fg.or(self.fg);
		self.bg = other.bg.or(self.bg);
		self.underline_color = other.underline_color.or(self.underline_color);
		self.underline_style = other.underline_style.or(self.underline_style);
		self.add_modifier.remove(other.sub_modifier);
		self.add_modifier.insert(other.add_modifier);
		self.sub_modifier.remove(other.add_modifier);
		self.sub_modifier.insert(other.sub_modifier);
		self
	}
}

impl From<Color> for Style {
	fn from(color: Color) -> Self {
		Style::new().fg(color)
	}
}

impl From<Modifier> for Style {
	fn from(modifier: Modifier) -> Self {
		Style::new().add_modifier(modifier)
	}
}

#[cfg(feature = "tui-style")]
impl From<Color> for xeno_tui::style::Color {
	fn from(value: Color) -> Self {
		match value {
			Color::Reset => xeno_tui::style::Color::Reset,
			Color::Black => xeno_tui::style::Color::Black,
			Color::Red => xeno_tui::style::Color::Red,
			Color::Green => xeno_tui::style::Color::Green,
			Color::Yellow => xeno_tui::style::Color::Yellow,
			Color::Blue => xeno_tui::style::Color::Blue,
			Color::Magenta => xeno_tui::style::Color::Magenta,
			Color::Cyan => xeno_tui::style::Color::Cyan,
			Color::Gray => xeno_tui::style::Color::Gray,
			Color::DarkGray => xeno_tui::style::Color::DarkGray,
			Color::LightRed => xeno_tui::style::Color::LightRed,
			Color::LightGreen => xeno_tui::style::Color::LightGreen,
			Color::LightYellow => xeno_tui::style::Color::LightYellow,
			Color::LightBlue => xeno_tui::style::Color::LightBlue,
			Color::LightMagenta => xeno_tui::style::Color::LightMagenta,
			Color::LightCyan => xeno_tui::style::Color::LightCyan,
			Color::White => xeno_tui::style::Color::White,
			Color::Rgb(r, g, b) => xeno_tui::style::Color::Rgb(r, g, b),
			Color::Indexed(index) => xeno_tui::style::Color::Indexed(index),
		}
	}
}

#[cfg(feature = "tui-style")]
impl From<xeno_tui::style::Color> for Color {
	fn from(value: xeno_tui::style::Color) -> Self {
		match value {
			xeno_tui::style::Color::Reset => Color::Reset,
			xeno_tui::style::Color::Black => Color::Black,
			xeno_tui::style::Color::Red => Color::Red,
			xeno_tui::style::Color::Green => Color::Green,
			xeno_tui::style::Color::Yellow => Color::Yellow,
			xeno_tui::style::Color::Blue => Color::Blue,
			xeno_tui::style::Color::Magenta => Color::Magenta,
			xeno_tui::style::Color::Cyan => Color::Cyan,
			xeno_tui::style::Color::Gray => Color::Gray,
			xeno_tui::style::Color::DarkGray => Color::DarkGray,
			xeno_tui::style::Color::LightRed => Color::LightRed,
			xeno_tui::style::Color::LightGreen => Color::LightGreen,
			xeno_tui::style::Color::LightYellow => Color::LightYellow,
			xeno_tui::style::Color::LightBlue => Color::LightBlue,
			xeno_tui::style::Color::LightMagenta => Color::LightMagenta,
			xeno_tui::style::Color::LightCyan => Color::LightCyan,
			xeno_tui::style::Color::White => Color::White,
			xeno_tui::style::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
			xeno_tui::style::Color::Indexed(index) => Color::Indexed(index),
		}
	}
}

#[cfg(feature = "tui-style")]
impl From<UnderlineStyle> for xeno_tui::style::UnderlineStyle {
	fn from(value: UnderlineStyle) -> Self {
		match value {
			UnderlineStyle::Reset => xeno_tui::style::UnderlineStyle::Reset,
			UnderlineStyle::Line => xeno_tui::style::UnderlineStyle::Line,
			UnderlineStyle::Curl => xeno_tui::style::UnderlineStyle::Curl,
			UnderlineStyle::Dotted => xeno_tui::style::UnderlineStyle::Dotted,
			UnderlineStyle::Dashed => xeno_tui::style::UnderlineStyle::Dashed,
			UnderlineStyle::DoubleLine => xeno_tui::style::UnderlineStyle::DoubleLine,
		}
	}
}

#[cfg(feature = "tui-style")]
impl From<Modifier> for xeno_tui::style::Modifier {
	fn from(value: Modifier) -> Self {
		xeno_tui::style::Modifier::from_bits_truncate(value.bits())
	}
}

#[cfg(feature = "tui-style")]
impl From<Style> for xeno_tui::style::Style {
	fn from(value: Style) -> Self {
		let mut style = xeno_tui::style::Style::new();
		if let Some(fg) = value.fg {
			style = style.fg(fg.into());
		}
		if let Some(bg) = value.bg {
			style = style.bg(bg.into());
		}
		if let Some(underline_color) = value.underline_color {
			style = style.underline_color(underline_color.into());
		}
		if let Some(underline_style) = value.underline_style {
			style = style.underline_style(underline_style.into());
		}
		style = style.add_modifier(value.add_modifier.into());
		style.remove_modifier(value.sub_modifier.into())
	}
}

#[cfg(test)]
mod tests {
	use super::Color;

	#[test]
	fn blend_alpha_preserves_existing_semantics() {
		let fg = Color::Rgb(200, 0, 0);
		let bg = Color::Rgb(0, 0, 200);
		assert_eq!(fg.blend(bg, 0.0), Color::Rgb(0, 0, 200));
		assert_eq!(fg.blend(bg, 1.0), Color::Rgb(200, 0, 0));
	}

	#[test]
	fn contrast_ratio_black_white_is_wcag_max() {
		let ratio = Color::Black.contrast_ratio(Color::White);
		assert!((ratio - 21.0).abs() < 0.1, "ratio={ratio}");
	}

	#[test]
	fn ensure_min_contrast_is_noop_when_already_sufficient() {
		let fg = Color::Rgb(180, 180, 180);
		let bg = Color::Rgb(20, 20, 20);
		let adjusted = fg.ensure_min_contrast(bg, 1.5);
		assert_eq!(adjusted.to_rgb(), fg.to_rgb());
	}

	#[test]
	fn ensure_min_contrast_boosts_when_needed() {
		let fg = Color::Rgb(40, 40, 40);
		let bg = Color::Rgb(30, 30, 30);
		let adjusted = fg.ensure_min_contrast(bg, 1.5);
		assert!(adjusted.contrast_ratio(bg) >= 1.5);
	}
}
