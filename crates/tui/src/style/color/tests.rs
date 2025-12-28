//! Tests for the `Color` type.

use super::*;

mod conversions;
mod display;
#[cfg(feature = "palette")]
mod palette;
mod parsing;
#[cfg(feature = "serde")]
mod serde;

#[test]
fn from_u32() {
	assert_eq!(Color::from_u32(0x000000), Color::Rgb(0, 0, 0));
	assert_eq!(Color::from_u32(0xFF0000), Color::Rgb(255, 0, 0));
	assert_eq!(Color::from_u32(0x00FF00), Color::Rgb(0, 255, 0));
	assert_eq!(Color::from_u32(0x0000FF), Color::Rgb(0, 0, 255));
	assert_eq!(Color::from_u32(0xFFFFFF), Color::Rgb(255, 255, 255));
}
