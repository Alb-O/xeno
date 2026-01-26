//! Tests for palette feature HSL/HSLuv conversions.

use palette::{Hsl, Hsluv};
use rstest::rstest;

use super::*;

#[rstest]
#[case::black(Hsl::new(0.0, 0.0, 0.0), Color::Rgb(0, 0, 0))]
#[case::white(Hsl::new(0.0, 0.0, 1.0), Color::Rgb(255, 255, 255))]
#[case::valid(Hsl::new(120.0, 0.5, 0.75), Color::Rgb(159, 223, 159))]
#[case::min_hue(Hsl::new(-180.0, 0.5, 0.75), Color::Rgb(159, 223, 223))]
#[case::max_hue(Hsl::new(180.0, 0.5, 0.75), Color::Rgb(159, 223, 223))]
#[case::min_saturation(Hsl::new(0.0, 0.0, 0.5), Color::Rgb(128, 128, 128))]
#[case::max_saturation(Hsl::new(0.0, 1.0, 0.5), Color::Rgb(255, 0, 0))]
#[case::min_lightness(Hsl::new(0.0, 0.5, 0.0), Color::Rgb(0, 0, 0))]
#[case::max_lightness(Hsl::new(0.0, 0.5, 1.0), Color::Rgb(255, 255, 255))]
#[case::under_hue_wraps(Hsl::new(-240.0, 0.5, 0.75), Color::Rgb(159, 223, 159))]
#[case::over_hue_wraps(Hsl::new(480.0, 0.5, 0.75), Color::Rgb(159, 223, 159))]
#[case::under_saturation_clamps(Hsl::new(0.0, -0.5, 0.75), Color::Rgb(191, 191, 191))]
#[case::over_saturation_clamps(Hsl::new(0.0, 1.2, 0.75), Color::Rgb(255, 128, 128))]
#[case::under_lightness_clamps(Hsl::new(0.0, 0.5, -0.20), Color::Rgb(0, 0, 0))]
#[case::over_lightness_clamps(Hsl::new(0.0, 0.5, 1.5), Color::Rgb(255, 255, 255))]
#[case::under_saturation_lightness_clamps(Hsl::new(0.0, -0.5, -0.20), Color::Rgb(0, 0, 0))]
#[case::over_saturation_lightness_clamps(Hsl::new(0.0, 1.2, 1.5), Color::Rgb(255, 255, 255))]
fn test_hsl_to_rgb(#[case] hsl: Hsl, #[case] expected: Color) {
	assert_eq!(Color::from_hsl(hsl), expected);
}

#[rstest]
#[case::black(Hsluv::new(0.0, 0.0, 0.0), Color::Rgb(0, 0, 0))]
#[case::white(Hsluv::new(0.0, 0.0, 100.0), Color::Rgb(255, 255, 255))]
#[case::valid(Hsluv::new(120.0, 50.0, 75.0), Color::Rgb(147, 198, 129))]
#[case::min_hue(Hsluv::new(-180.0, 50.0, 75.0), Color::Rgb(135,196, 188))]
#[case::max_hue(Hsluv::new(180.0, 50.0, 75.0), Color::Rgb(135, 196, 188))]
#[case::min_saturation(Hsluv::new(0.0, 0.0, 75.0), Color::Rgb(185, 185, 185))]
#[case::max_saturation(Hsluv::new(0.0, 100.0, 75.0), Color::Rgb(255, 156, 177))]
#[case::min_lightness(Hsluv::new(0.0, 50.0, 0.0), Color::Rgb(0, 0, 0))]
#[case::max_lightness(Hsluv::new(0.0, 50.0, 100.0), Color::Rgb(255, 255, 255))]
#[case::under_hue_wraps(Hsluv::new(-240.0, 50.0, 75.0), Color::Rgb(147, 198, 129))]
#[case::over_hue_wraps(Hsluv::new(480.0, 50.0, 75.0), Color::Rgb(147, 198, 129))]
#[case::under_saturation_clamps(Hsluv::new(0.0, -50.0, 75.0), Color::Rgb(185, 185, 185))]
#[case::over_saturation_clamps(Hsluv::new(0.0, 150.0, 75.0), Color::Rgb(255, 156, 177))]
#[case::under_lightness_clamps(Hsluv::new(0.0, 50.0, -20.0), Color::Rgb(0, 0, 0))]
#[case::over_lightness_clamps(Hsluv::new(0.0, 50.0, 150.0), Color::Rgb(255, 255, 255))]
#[case::under_saturation_lightness_clamps(Hsluv::new(0.0, -50.0, -20.0), Color::Rgb(0, 0, 0))]
#[case::over_saturation_lightness_clamps(Hsluv::new(0.0, 150.0, 150.0), Color::Rgb(255, 255, 255))]
fn test_hsluv_to_rgb(#[case] hsluv: Hsluv, #[case] expected: Color) {
	assert_eq!(Color::from_hsluv(hsluv), expected);
}
