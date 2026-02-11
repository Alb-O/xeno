//! Tests for the `Color` type.

use super::*;

mod conversions;
mod display;
#[cfg(feature = "palette")]
mod palette_tests;
mod parsing;
#[cfg(feature = "serde")]
mod serde_tests;

#[test]
fn from_u32() {
	assert_eq!(Color::from_u32(0x000000), Color::Rgb(0, 0, 0));
	assert_eq!(Color::from_u32(0xFF0000), Color::Rgb(255, 0, 0));
	assert_eq!(Color::from_u32(0x00FF00), Color::Rgb(0, 255, 0));
	assert_eq!(Color::from_u32(0x0000FF), Color::Rgb(0, 0, 255));
	assert_eq!(Color::from_u32(0xFFFFFF), Color::Rgb(255, 255, 255));
}

#[test]
fn luminance_extremes() {
	// Black has zero luminance
	assert!((Color::Black.luminance() - 0.0).abs() < 0.001);
	assert!((Color::Rgb(0, 0, 0).luminance() - 0.0).abs() < 0.001);

	// White has maximum luminance
	assert!((Color::White.luminance() - 1.0).abs() < 0.001);
	assert!((Color::Rgb(255, 255, 255).luminance() - 1.0).abs() < 0.001);
}

#[test]
fn luminance_mid_gray() {
	// Mid gray (sRGB 127) has luminance ~0.212 due to gamma correction
	let gray = Color::Rgb(127, 127, 127);
	let lum = gray.luminance();
	assert!(lum > 0.1 && lum < 0.3, "mid gray luminance was {lum}");
}

#[test]
fn luminance_color_weights() {
	// Green contributes most to luminance (0.7152 coefficient)
	let red = Color::Rgb(255, 0, 0).luminance();
	let green = Color::Rgb(0, 255, 0).luminance();
	let blue = Color::Rgb(0, 0, 255).luminance();

	assert!(green > red, "green ({green}) should be brighter than red ({red})");
	assert!(red > blue, "red ({red}) should be brighter than blue ({blue})");
}

#[test]
fn contrast_ratio_extremes() {
	// Black vs white = maximum contrast (21:1)
	let ratio = Color::Black.contrast_ratio(Color::White);
	assert!((ratio - 21.0).abs() < 0.1, "black/white contrast was {ratio}");

	// Same color = minimum contrast (1:1)
	let ratio = Color::Blue.contrast_ratio(Color::Blue);
	assert!((ratio - 1.0).abs() < 0.001, "same color contrast was {ratio}");
}

#[test]
fn contrast_ratio_symmetry() {
	// Contrast ratio is symmetric
	let a = Color::Rgb(100, 50, 200);
	let b = Color::Rgb(200, 180, 50);
	let ratio_ab = a.contrast_ratio(b);
	let ratio_ba = b.contrast_ratio(a);
	assert!((ratio_ab - ratio_ba).abs() < 0.001, "contrast should be symmetric: {ratio_ab} vs {ratio_ba}");
}

#[test]
fn ensure_min_contrast_no_change_when_sufficient() {
	let fg = Color::White;
	let bg = Color::Black;

	// Already high contrast, should return unchanged
	let result = fg.ensure_min_contrast(bg, 1.5);
	assert_eq!(result.to_rgb(), fg.to_rgb());
}

#[test]
fn ensure_min_contrast_boosts_on_dark_bg() {
	let dark_bg = Color::Rgb(30, 30, 30);
	let too_dark = Color::Rgb(40, 40, 40);

	// Initial contrast is very low
	let initial_ratio = too_dark.contrast_ratio(dark_bg);
	assert!(initial_ratio < 1.2, "initial contrast should be low: {initial_ratio}");

	// After boosting, contrast meets minimum
	let boosted = too_dark.ensure_min_contrast(dark_bg, 1.5);
	let boosted_ratio = boosted.contrast_ratio(dark_bg);
	assert!(boosted_ratio >= 1.5, "boosted contrast {boosted_ratio} should be >= 1.5");

	// Should have shifted toward white (higher RGB values)
	let (r, g, b) = boosted.to_rgb();
	assert!(r > 40 && g > 40 && b > 40, "should shift toward white on dark bg");
}

#[test]
fn ensure_min_contrast_boosts_on_light_bg() {
	let light_bg = Color::Rgb(230, 230, 230);
	let too_light = Color::Rgb(220, 220, 220);

	// Initial contrast is very low
	let initial_ratio = too_light.contrast_ratio(light_bg);
	assert!(initial_ratio < 1.2, "initial contrast should be low: {initial_ratio}");

	// After boosting, contrast meets minimum
	let boosted = too_light.ensure_min_contrast(light_bg, 1.5);
	let boosted_ratio = boosted.contrast_ratio(light_bg);
	assert!(boosted_ratio >= 1.5, "boosted contrast {boosted_ratio} should be >= 1.5");

	// Should have shifted toward black (lower RGB values)
	let (r, g, b) = boosted.to_rgb();
	assert!(r < 220 && g < 220 && b < 220, "should shift toward black on light bg");
}
