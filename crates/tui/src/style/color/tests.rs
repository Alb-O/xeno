//! Tests for the `Color` type.

use super::*;

#[path = "conversions.rs"]
mod conversions;
#[path = "display.rs"]
mod display;
#[cfg(feature = "palette")]
#[path = "palette_tests.rs"]
mod palette_tests;
#[path = "parsing.rs"]
mod parsing;
#[cfg(feature = "serde")]
#[path = "serde_tests.rs"]
mod serde_tests;

#[test]
fn from_u32() {
	assert_eq!(Color::from_u32(0x000000), Color::Rgb(0, 0, 0));
	assert_eq!(Color::from_u32(0xFF0000), Color::Rgb(255, 0, 0));
	assert_eq!(Color::from_u32(0x00FF00), Color::Rgb(0, 255, 0));
	assert_eq!(Color::from_u32(0x0000FF), Color::Rgb(0, 0, 255));
	assert_eq!(Color::from_u32(0xFFFFFF), Color::Rgb(255, 255, 255));
}
