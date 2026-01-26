//! Tests for parsing colors from strings.

use alloc::boxed::Box;
use core::error::Error;
use core::str::FromStr;

use super::*;

#[test]
fn from_rgb_color() {
	let color: Color = Color::from_str("#FF0000").unwrap();
	assert_eq!(color, Color::Rgb(255, 0, 0));
}

#[test]
fn from_indexed_color() {
	let color: Color = Color::from_str("10").unwrap();
	assert_eq!(color, Color::Indexed(10));
}

#[test]
fn from_ansi_color() -> Result<(), Box<dyn Error>> {
	assert_eq!(Color::from_str("reset")?, Color::Reset);
	assert_eq!(Color::from_str("black")?, Color::Black);
	assert_eq!(Color::from_str("red")?, Color::Red);
	assert_eq!(Color::from_str("green")?, Color::Green);
	assert_eq!(Color::from_str("yellow")?, Color::Yellow);
	assert_eq!(Color::from_str("blue")?, Color::Blue);
	assert_eq!(Color::from_str("magenta")?, Color::Magenta);
	assert_eq!(Color::from_str("cyan")?, Color::Cyan);
	assert_eq!(Color::from_str("gray")?, Color::Gray);
	assert_eq!(Color::from_str("darkgray")?, Color::DarkGray);
	assert_eq!(Color::from_str("lightred")?, Color::LightRed);
	assert_eq!(Color::from_str("lightgreen")?, Color::LightGreen);
	assert_eq!(Color::from_str("lightyellow")?, Color::LightYellow);
	assert_eq!(Color::from_str("lightblue")?, Color::LightBlue);
	assert_eq!(Color::from_str("lightmagenta")?, Color::LightMagenta);
	assert_eq!(Color::from_str("lightcyan")?, Color::LightCyan);
	assert_eq!(Color::from_str("white")?, Color::White);

	assert_eq!(Color::from_str("lightblack")?, Color::DarkGray);
	assert_eq!(Color::from_str("lightwhite")?, Color::White);
	assert_eq!(Color::from_str("lightgray")?, Color::White);

	assert_eq!(Color::from_str("grey")?, Color::Gray);
	assert_eq!(Color::from_str("silver")?, Color::Gray);

	assert_eq!(Color::from_str("light black")?, Color::DarkGray);
	assert_eq!(Color::from_str("light white")?, Color::White);
	assert_eq!(Color::from_str("light gray")?, Color::White);

	assert_eq!(Color::from_str("light-black")?, Color::DarkGray);
	assert_eq!(Color::from_str("light-white")?, Color::White);
	assert_eq!(Color::from_str("light-gray")?, Color::White);

	assert_eq!(Color::from_str("light_black")?, Color::DarkGray);
	assert_eq!(Color::from_str("light_white")?, Color::White);
	assert_eq!(Color::from_str("light_gray")?, Color::White);

	assert_eq!(Color::from_str("bright-black")?, Color::DarkGray);
	assert_eq!(Color::from_str("bright-white")?, Color::White);

	assert_eq!(Color::from_str("brightblack")?, Color::DarkGray);
	assert_eq!(Color::from_str("brightwhite")?, Color::White);

	Ok(())
}

#[test]
fn from_invalid_colors() {
	let bad_colors = [
		"invalid_color",
		"abcdef0",
		" bcdefa",
		"#abcdef00",
		"#1🦀2",
		"resets",
		"lightblackk",
	];

	for bad_color in bad_colors {
		assert!(
			Color::from_str(bad_color).is_err(),
			"bad color: '{bad_color}'"
		);
	}
}
