#![allow(clippy::unreadable_literal, reason = "hex color literals are more readable without underscores")]
//! ANSI color model plus parsing and conversion helpers.

use core::fmt;
use core::str::FromStr;

use crate::style::stylize::{ColorDebug, ColorDebugKind};

/// ANSI Color
///
/// All colors from the [ANSI color table] are supported (though some names are not exactly the
/// same).
///
/// | Color Name     | Color                   | Foreground | Background |
/// |----------------|-------------------------|------------|------------|
/// | `black`        | [`Color::Black`]        | 30         | 40         |
/// | `red`          | [`Color::Red`]          | 31         | 41         |
/// | `green`        | [`Color::Green`]        | 32         | 42         |
/// | `yellow`       | [`Color::Yellow`]       | 33         | 43         |
/// | `blue`         | [`Color::Blue`]         | 34         | 44         |
/// | `magenta`      | [`Color::Magenta`]      | 35         | 45         |
/// | `cyan`         | [`Color::Cyan`]         | 36         | 46         |
/// | `gray`*        | [`Color::Gray`]         | 37         | 47         |
/// | `darkgray`*    | [`Color::DarkGray`]     | 90         | 100        |
/// | `lightred`     | [`Color::LightRed`]     | 91         | 101        |
/// | `lightgreen`   | [`Color::LightGreen`]   | 92         | 102        |
/// | `lightyellow`  | [`Color::LightYellow`]  | 93         | 103        |
/// | `lightblue`    | [`Color::LightBlue`]    | 94         | 104        |
/// | `lightmagenta` | [`Color::LightMagenta`] | 95         | 105        |
/// | `lightcyan`    | [`Color::LightCyan`]    | 96         | 106        |
/// | `white`*       | [`Color::White`]        | 97         | 107        |
///
/// * `gray` is sometimes called `white` - this is not supported as we use `white` for bright white
/// * `gray` is sometimes called `silver` - this is supported
/// * `darkgray` is sometimes called `light black` or `bright black` (both are supported)
/// * `white` is sometimes called `light white` or `bright white` (both are supported)
/// * we support `bright` and `light` prefixes for all colors
/// * we support `-` and `_` and ` ` as separators for all colors
/// * we support both `gray` and `grey` spellings
///
/// `From<Color> for Style` is implemented by creating a style with the foreground color set to the
/// given color. This allows you to use colors anywhere that accepts `Into<Style>`.
///
/// # Example
///
/// ```
/// use std::str::FromStr;
///
/// use xeno_tui::style::Color;
///
/// assert_eq!(Color::from_str("red"), Ok(Color::Red));
/// assert_eq!("red".parse(), Ok(Color::Red));
/// assert_eq!("lightred".parse(), Ok(Color::LightRed));
/// assert_eq!("light red".parse(), Ok(Color::LightRed));
/// assert_eq!("light-red".parse(), Ok(Color::LightRed));
/// assert_eq!("light_red".parse(), Ok(Color::LightRed));
/// assert_eq!("lightRed".parse(), Ok(Color::LightRed));
/// assert_eq!("bright red".parse(), Ok(Color::LightRed));
/// assert_eq!("bright-red".parse(), Ok(Color::LightRed));
/// assert_eq!("silver".parse(), Ok(Color::Gray));
/// assert_eq!("dark-grey".parse(), Ok(Color::DarkGray));
/// assert_eq!("dark gray".parse(), Ok(Color::DarkGray));
/// assert_eq!("light-black".parse(), Ok(Color::DarkGray));
/// assert_eq!("white".parse(), Ok(Color::White));
/// assert_eq!("bright white".parse(), Ok(Color::White));
/// ```
///
/// [ANSI color table]: https://en.wikipedia.org/wiki/ANSI_escape_code#Colors
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Color {
	/// Resets the foreground or background color
	#[default]
	Reset,
	/// ANSI Color: Black. Foreground: 30, Background: 40
	Black,
	/// ANSI Color: Red. Foreground: 31, Background: 41
	Red,
	/// ANSI Color: Green. Foreground: 32, Background: 42
	Green,
	/// ANSI Color: Yellow. Foreground: 33, Background: 43
	Yellow,
	/// ANSI Color: Blue. Foreground: 34, Background: 44
	Blue,
	/// ANSI Color: Magenta. Foreground: 35, Background: 45
	Magenta,
	/// ANSI Color: Cyan. Foreground: 36, Background: 46
	Cyan,
	/// ANSI Color: White. Foreground: 37, Background: 47
	///
	/// Note that this is sometimes called `silver` or `white` but we use `white` for bright white
	Gray,
	/// ANSI Color: Bright Black. Foreground: 90, Background: 100
	///
	/// Note that this is sometimes called `light black` or `bright black` but we use `dark gray`
	DarkGray,
	/// ANSI Color: Bright Red. Foreground: 91, Background: 101
	LightRed,
	/// ANSI Color: Bright Green. Foreground: 92, Background: 102
	LightGreen,
	/// ANSI Color: Bright Yellow. Foreground: 93, Background: 103
	LightYellow,
	/// ANSI Color: Bright Blue. Foreground: 94, Background: 104
	LightBlue,
	/// ANSI Color: Bright Magenta. Foreground: 95, Background: 105
	LightMagenta,
	/// ANSI Color: Bright Cyan. Foreground: 96, Background: 106
	LightCyan,
	/// ANSI Color: Bright White. Foreground: 97, Background: 107
	/// Sometimes called `bright white` or `light white` in some terminals
	White,
	/// An RGB color.
	///
	/// Note that only terminals that support 24-bit true color will display this correctly.
	/// Notably versions of Windows Terminal prior to Windows 10 and macOS Terminal.app do not
	/// support this.
	///
	/// If the terminal does not support true color, code using the  `TermwizBackend` will
	/// fallback to the default text color. Crossterm and Termion do not have this capability and
	/// the display will be unpredictable (e.g. Terminal.app may display glitched blinking text).
	/// See  for an example of this problem.
	///
	/// See also: <https://en.wikipedia.org/wiki/ANSI_escape_code#24-bit>
	Rgb(u8, u8, u8),
	/// An 8-bit 256 color.
	///
	/// See also <https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit>
	Indexed(u8),
}

impl Color {
	/// Convert a u32 to a Color
	///
	/// The u32 should be in the format 0x00RRGGBB.
	pub const fn from_u32(u: u32) -> Self {
		let r = (u >> 16) as u8;
		let g = (u >> 8) as u8;
		let b = u as u8;
		Self::Rgb(r, g, b)
	}
}

#[cfg(feature = "serde")]
impl serde::Serialize for Color {
	/// This utilises the [`fmt::Display`] implementation for serialization.
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_str(&self.to_string())
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Color {
	/// This is used to deserialize a value into Color via serde.
	///
	/// This implementation uses the `FromStr` trait to deserialize strings, so named colours, RGB,
	/// and indexed values are able to be deserialized.
	///
	/// See the [`Color`] documentation for more information on color names.
	///
	/// # Examples
	///
	/// ```
	/// use std::str::FromStr;
	///
	/// use xeno_tui::style::Color;
	///
	/// #[derive(Debug, serde::Deserialize)]
	/// struct Theme {
	///     color: Color,
	/// }
	///
	/// # fn get_theme() -> Result<(), serde_json::Error> {
	/// let theme: Theme = serde_json::from_str(r#"{"color": "bright-white"}"#)?;
	/// assert_eq!(theme.color, Color::White);
	///
	/// let theme: Theme = serde_json::from_str(r##"{"color": "#00FF00"}"##)?;
	/// assert_eq!(theme.color, Color::Rgb(0, 255, 0));
	///
	/// let theme: Theme = serde_json::from_str(r#"{"color": "42"}"#)?;
	/// assert_eq!(theme.color, Color::Indexed(42));
	///
	/// let err = serde_json::from_str::<Theme>(r#"{"color": "invalid"}"#).unwrap_err();
	/// assert!(err.is_data());
	/// assert_eq!(
	///     err.to_string(),
	///     "Failed to parse Colors at line 1 column 20"
	/// );
	///
	/// # Ok(())
	/// # }
	/// ```
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let value =
			<String as serde::Deserialize>::deserialize(deserializer).map_err(|err| serde::de::Error::custom(format!("Failed to parse Colors: {err}")))?;
		FromStr::from_str(&value).map_err(serde::de::Error::custom)
	}
}

/// Error type indicating a failure to parse a color string.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ParseColorError;

impl fmt::Display for ParseColorError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Failed to parse Colors")
	}
}

impl core::error::Error for ParseColorError {}

/// Converts a string representation to a `Color` instance.
///
/// The `from_str` function attempts to parse the given string and convert it to the corresponding
/// `Color` variant. It supports named colors, RGB values, and indexed colors. If the string cannot
/// be parsed, a `ParseColorError` is returned.
///
/// See the [`Color`] documentation for more information on the supported color names.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use xeno_tui::style::Color;
///
/// let color: Color = Color::from_str("blue").unwrap();
/// assert_eq!(color, Color::Blue);
///
/// let color: Color = Color::from_str("#FF0000").unwrap();
/// assert_eq!(color, Color::Rgb(255, 0, 0));
///
/// let color: Color = Color::from_str("10").unwrap();
/// assert_eq!(color, Color::Indexed(10));
///
/// let color: Result<Color, _> = Color::from_str("invalid_color");
/// assert!(color.is_err());
/// ```
impl FromStr for Color {
	type Err = ParseColorError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(
			// There is a mix of different color names and formats in the wild.
			// This is an attempt to support as many as possible.
			match s
				.to_lowercase()
				.replace([' ', '-', '_'], "")
				.replace("bright", "light")
				.replace("grey", "gray")
				.replace("silver", "gray")
				.replace("lightblack", "darkgray")
				.replace("lightwhite", "white")
				.replace("lightgray", "white")
				.as_ref()
			{
				"reset" => Self::Reset,
				"black" => Self::Black,
				"red" => Self::Red,
				"green" => Self::Green,
				"yellow" => Self::Yellow,
				"blue" => Self::Blue,
				"magenta" => Self::Magenta,
				"cyan" => Self::Cyan,
				"gray" => Self::Gray,
				"darkgray" => Self::DarkGray,
				"lightred" => Self::LightRed,
				"lightgreen" => Self::LightGreen,
				"lightyellow" => Self::LightYellow,
				"lightblue" => Self::LightBlue,
				"lightmagenta" => Self::LightMagenta,
				"lightcyan" => Self::LightCyan,
				"white" => Self::White,
				_ => {
					if let Ok(index) = s.parse::<u8>() {
						Self::Indexed(index)
					} else if let Some((r, g, b)) = parse_hex_color(s) {
						Self::Rgb(r, g, b)
					} else {
						return Err(ParseColorError);
					}
				}
			},
		)
	}
}

/// Parses a hex color string (e.g., "#ff0000") into RGB components.
fn parse_hex_color(input: &str) -> Option<(u8, u8, u8)> {
	if !input.starts_with('#') || input.len() != 7 {
		return None;
	}
	let r = u8::from_str_radix(input.get(1..3)?, 16).ok()?;
	let g = u8::from_str_radix(input.get(3..5)?, 16).ok()?;
	let b = u8::from_str_radix(input.get(5..7)?, 16).ok()?;
	Some((r, g, b))
}

impl fmt::Display for Color {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Reset => write!(f, "Reset"),
			Self::Black => write!(f, "Black"),
			Self::Red => write!(f, "Red"),
			Self::Green => write!(f, "Green"),
			Self::Yellow => write!(f, "Yellow"),
			Self::Blue => write!(f, "Blue"),
			Self::Magenta => write!(f, "Magenta"),
			Self::Cyan => write!(f, "Cyan"),
			Self::Gray => write!(f, "Gray"),
			Self::DarkGray => write!(f, "DarkGray"),
			Self::LightRed => write!(f, "LightRed"),
			Self::LightGreen => write!(f, "LightGreen"),
			Self::LightYellow => write!(f, "LightYellow"),
			Self::LightBlue => write!(f, "LightBlue"),
			Self::LightMagenta => write!(f, "LightMagenta"),
			Self::LightCyan => write!(f, "LightCyan"),
			Self::White => write!(f, "White"),
			Self::Rgb(r, g, b) => write!(f, "#{r:02X}{g:02X}{b:02X}"),
			Self::Indexed(i) => write!(f, "{i}"),
		}
	}
}

impl Color {
	/// Wraps this color for debug formatting with the given kind.
	pub(crate) const fn stylize_debug(self, kind: ColorDebugKind) -> ColorDebug {
		ColorDebug { kind, color: self }
	}

	/// Converts a HSL representation to a `Color::Rgb` instance.
	///
	/// The `from_hsl` function converts the Hue, Saturation and Lightness values to a corresponding
	/// `Color` RGB equivalent.
	///
	/// Hue values should be in the range [-180..180]. Values outside this range are normalized by
	/// wrapping.
	///
	/// Saturation and L values should be in the range [0.0..1.0]. Values outside this range are
	/// clamped.
	///
	/// Clamping to valid ranges happens before conversion to RGB.
	///
	/// # Examples
	///
	/// ```
	/// use palette::Hsl;
	///
	/// use xeno_tui::style::Color;
	///
	/// // Minimum Lightness is black
	/// let color: Color = Color::from_hsl(Hsl::new(0.0, 0.0, 0.0));
	/// assert_eq!(color, Color::Rgb(0, 0, 0));
	///
	/// // Maximum Lightness is white
	/// let color: Color = Color::from_hsl(Hsl::new(0.0, 0.0, 1.0));
	/// assert_eq!(color, Color::Rgb(255, 255, 255));
	///
	/// // Minimum Saturation is fully desaturated red = gray
	/// let color: Color = Color::from_hsl(Hsl::new(0.0, 0.0, 0.5));
	/// assert_eq!(color, Color::Rgb(128, 128, 128));
	///
	/// // Bright red
	/// let color: Color = Color::from_hsl(Hsl::new(0.0, 1.0, 0.5));
	/// assert_eq!(color, Color::Rgb(255, 0, 0));
	///
	/// // Bright blue
	/// let color: Color = Color::from_hsl(Hsl::new(-120.0, 1.0, 0.5));
	/// assert_eq!(color, Color::Rgb(0, 0, 255));
	/// ```
	#[cfg(feature = "palette")]
	pub fn from_hsl(hsl: palette::Hsl) -> Self {
		use palette::{Clamp, FromColor, Srgb};
		let hsl = hsl.clamp();
		let Srgb { red, green, blue, standard: _ }: Srgb<u8> = Srgb::from_color(hsl).into();

		Self::Rgb(red, green, blue)
	}

	/// Converts a `HSLuv` representation to a `Color::Rgb` instance.
	///
	/// The `from_hsluv` function converts the Hue, Saturation and Lightness values to a
	/// corresponding `Color` RGB equivalent.
	///
	/// Hue values should be in the range [-180.0..180.0]. Values outside this range are normalized
	/// by wrapping.
	///
	/// Saturation and L values should be in the range [0.0..100.0]. Values outside this range are
	/// clamped.
	///
	/// Clamping to valid ranges happens before conversion to RGB.
	///
	/// # Examples
	///
	/// ```
	/// use palette::Hsluv;
	///
	/// use xeno_tui::style::Color;
	///
	/// // Minimum Lightness is black
	/// let color: Color = Color::from_hsluv(Hsluv::new(0.0, 100.0, 0.0));
	/// assert_eq!(color, Color::Rgb(0, 0, 0));
	///
	/// // Maximum Lightness is white
	/// let color: Color = Color::from_hsluv(Hsluv::new(0.0, 0.0, 100.0));
	/// assert_eq!(color, Color::Rgb(255, 255, 255));
	///
	/// // Minimum Saturation is fully desaturated red = gray
	/// let color = Color::from_hsluv(Hsluv::new(0.0, 0.0, 50.0));
	/// assert_eq!(color, Color::Rgb(119, 119, 119));
	///
	/// // Bright Red
	/// let color = Color::from_hsluv(Hsluv::new(12.18, 100.0, 53.2));
	/// assert_eq!(color, Color::Rgb(255, 0, 0));
	///
	/// // Bright Blue
	/// let color = Color::from_hsluv(Hsluv::new(-94.13, 100.0, 32.3));
	/// assert_eq!(color, Color::Rgb(0, 0, 255));
	/// ```
	#[cfg(feature = "palette")]
	pub fn from_hsluv(hsluv: palette::Hsluv) -> Self {
		use palette::{Clamp, FromColor, Srgb};
		let hsluv = hsluv.clamp();
		let Srgb { red, green, blue, standard: _ }: Srgb<u8> = Srgb::from_color(hsluv).into();

		Self::Rgb(red, green, blue)
	}

	/// Converts the color to RGB components.
	///
	/// For ANSI colors, returns the standard VGA palette values.
	/// For `Reset`, returns black (0, 0, 0).
	/// For `Indexed` colors 0-15, returns standard ANSI colors.
	/// For `Indexed` colors 16-231, returns the 6x6x6 color cube values.
	/// For `Indexed` colors 232-255, returns grayscale values.
	///
	/// # Example
	///
	/// ```
	/// use xeno_tui::style::Color;
	///
	/// assert_eq!(Color::Red.to_rgb(), (128, 0, 0));
	/// assert_eq!(Color::Rgb(255, 128, 0).to_rgb(), (255, 128, 0));
	/// ```
	pub const fn to_rgb(self) -> (u8, u8, u8) {
		match self {
			Color::Reset => (0, 0, 0),
			Color::Black => (0, 0, 0),
			Color::Red => (128, 0, 0),
			Color::Green => (0, 128, 0),
			Color::Yellow => (128, 128, 0),
			Color::Blue => (0, 0, 128),
			Color::Magenta => (128, 0, 128),
			Color::Cyan => (0, 128, 128),
			Color::Gray => (192, 192, 192),
			Color::DarkGray => (128, 128, 128),
			Color::LightRed => (255, 0, 0),
			Color::LightGreen => (0, 255, 0),
			Color::LightYellow => (255, 255, 0),
			Color::LightBlue => (0, 0, 255),
			Color::LightMagenta => (255, 0, 255),
			Color::LightCyan => (0, 255, 255),
			Color::White => (255, 255, 255),
			Color::Rgb(r, g, b) => (r, g, b),
			Color::Indexed(idx) => indexed_to_rgb(idx),
		}
	}

	/// Blend this color with another using alpha compositing.
	///
	/// When `alpha` is 0.0, returns `other`. When `alpha` is 1.0, returns `self`.
	/// Both colors are converted to RGB for blending.
	pub fn blend(self, other: Self, alpha: f32) -> Self {
		let (r1, g1, b1) = self.to_rgb();
		let (r2, g2, b2) = other.to_rgb();
		let alpha = alpha.clamp(0.0, 1.0);
		let blend = |a: u8, b: u8| (a as f32 * alpha + b as f32 * (1.0 - alpha)).round() as u8;
		Self::Rgb(blend(r1, r2), blend(g1, g2), blend(b1, b2))
	}

	/// Computes relative luminance per WCAG 2.1 specification.
	///
	/// Returns a value in the range `0.0` (black) to `1.0` (white).
	/// Converts sRGB to linear RGB before applying the standard luminance
	/// coefficients (0.2126 R + 0.7152 G + 0.0722 B).
	///
	/// See: <https://www.w3.org/TR/WCAG21/#dfn-relative-luminance>
	///
	/// # Examples
	///
	/// ```
	/// use xeno_tui::style::Color;
	///
	/// assert!((Color::Black.luminance() - 0.0).abs() < 0.001);
	/// assert!((Color::White.luminance() - 1.0).abs() < 0.001);
	/// ```
	pub fn luminance(self) -> f32 {
		let (r, g, b) = self.to_rgb();
		let to_linear = |c: u8| {
			let c = c as f32 / 255.0;
			if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
		};
		0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
	}

	/// Computes WCAG contrast ratio against `other`.
	///
	/// Returns a value in the range `1.0` (identical) to `21.0` (black vs white).
	/// WCAG recommends minimum 3:1 for large UI elements, 4.5:1 for normal text.
	///
	/// See: <https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio>
	///
	/// # Examples
	///
	/// ```
	/// use xeno_tui::style::Color;
	///
	/// let ratio = Color::Black.contrast_ratio(Color::White);
	/// assert!((ratio - 21.0).abs() < 0.1);
	///
	/// let ratio = Color::Blue.contrast_ratio(Color::Blue);
	/// assert!((ratio - 1.0).abs() < 0.001);
	/// ```
	pub fn contrast_ratio(self, other: Self) -> f32 {
		let l1 = self.luminance();
		let l2 = other.luminance();
		let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
		(lighter + 0.05) / (darker + 0.05)
	}

	/// Ensures minimum contrast against a `background` color.
	///
	/// Returns `self` unchanged if contrast already meets `min_ratio`.
	/// Otherwise, blends toward white (for dark backgrounds) or black
	/// (for light backgrounds) using binary search to find the minimal
	/// adjustment that achieves the target contrast ratio.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_tui::style::Color;
	///
	/// let dark_bg = Color::Rgb(30, 30, 30);
	/// let too_similar = Color::Rgb(40, 40, 40);
	///
	/// let boosted = too_similar.ensure_min_contrast(dark_bg, 1.5);
	/// assert!(boosted.contrast_ratio(dark_bg) >= 1.5);
	/// ```
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

/// Converts an indexed color (0-255) to RGB.
const fn indexed_to_rgb(idx: u8) -> (u8, u8, u8) {
	match idx {
		// Standard ANSI colors (0-15)
		0 => (0, 0, 0),
		1 => (128, 0, 0),
		2 => (0, 128, 0),
		3 => (128, 128, 0),
		4 => (0, 0, 128),
		5 => (128, 0, 128),
		6 => (0, 128, 128),
		7 => (192, 192, 192),
		8 => (128, 128, 128),
		9 => (255, 0, 0),
		10 => (0, 255, 0),
		11 => (255, 255, 0),
		12 => (0, 0, 255),
		13 => (255, 0, 255),
		14 => (0, 255, 255),
		15 => (255, 255, 255),
		// 6x6x6 color cube (16-231). Each channel maps to: 0, 95, 135, 175, 215, 255.
		16..=231 => {
			let idx = idx - 16;
			let ri = idx / 36;
			let gi = (idx % 36) / 6;
			let bi = idx % 6;
			let r = if ri == 0 { 0 } else { 55 + ri * 40 };
			let g = if gi == 0 { 0 } else { 55 + gi * 40 };
			let b = if bi == 0 { 0 } else { 55 + bi * 40 };
			(r, g, b)
		}
		232..=255 => {
			let gray = 8 + (idx - 232) * 10;
			(gray, gray, gray)
		}
	}
}

impl crate::animation::Animatable for Color {
	/// Linearly interpolate between two colors.
	///
	/// Converts both colors to RGB, interpolates each component,
	/// and returns a new `Color::Rgb`.
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let (r1, g1, b1) = self.to_rgb();
		let (r2, g2, b2) = target.to_rgb();

		// Use the Animatable impl for u8 directly
		let t = t.clamp(0.0, 1.0);
		let lerp_u8 = |a: u8, b: u8| -> u8 {
			let result = a as f32 + (b as f32 - a as f32) * t;
			result.round() as u8
		};

		Color::Rgb(lerp_u8(r1, r2), lerp_u8(g1, g2), lerp_u8(b1, b2))
	}
}

impl From<[u8; 3]> for Color {
	/// Converts an array of 3 u8 values to a `Color::Rgb` instance.
	fn from([r, g, b]: [u8; 3]) -> Self {
		Self::Rgb(r, g, b)
	}
}

impl From<(u8, u8, u8)> for Color {
	/// Converts a tuple of 3 u8 values to a `Color::Rgb` instance.
	fn from((r, g, b): (u8, u8, u8)) -> Self {
		Self::Rgb(r, g, b)
	}
}

impl From<[u8; 4]> for Color {
	/// Converts an array of 4 u8 values to a `Color::Rgb` instance (ignoring the alpha value).
	fn from([r, g, b, _]: [u8; 4]) -> Self {
		Self::Rgb(r, g, b)
	}
}

impl From<(u8, u8, u8, u8)> for Color {
	/// Converts a tuple of 4 u8 values to a `Color::Rgb` instance (ignoring the alpha value).
	fn from((r, g, b, _): (u8, u8, u8, u8)) -> Self {
		Self::Rgb(r, g, b)
	}
}

#[cfg(test)]
mod tests;
