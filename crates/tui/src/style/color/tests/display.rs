//! Tests for Color Display implementation.

use super::*;

#[test]
fn display() {
	assert_eq!(format!("{}", Color::Black), "Black");
	assert_eq!(format!("{}", Color::Red), "Red");
	assert_eq!(format!("{}", Color::Green), "Green");
	assert_eq!(format!("{}", Color::Yellow), "Yellow");
	assert_eq!(format!("{}", Color::Blue), "Blue");
	assert_eq!(format!("{}", Color::Magenta), "Magenta");
	assert_eq!(format!("{}", Color::Cyan), "Cyan");
	assert_eq!(format!("{}", Color::Gray), "Gray");
	assert_eq!(format!("{}", Color::DarkGray), "DarkGray");
	assert_eq!(format!("{}", Color::LightRed), "LightRed");
	assert_eq!(format!("{}", Color::LightGreen), "LightGreen");
	assert_eq!(format!("{}", Color::LightYellow), "LightYellow");
	assert_eq!(format!("{}", Color::LightBlue), "LightBlue");
	assert_eq!(format!("{}", Color::LightMagenta), "LightMagenta");
	assert_eq!(format!("{}", Color::LightCyan), "LightCyan");
	assert_eq!(format!("{}", Color::White), "White");
	assert_eq!(format!("{}", Color::Indexed(10)), "10");
	assert_eq!(format!("{}", Color::Rgb(255, 0, 0)), "#FF0000");
	assert_eq!(format!("{}", Color::Reset), "Reset");
}
