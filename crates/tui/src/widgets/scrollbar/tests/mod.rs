use alloc::format;
use alloc::string::ToString;
use core::str::FromStr;

use rstest::fixture;
use strum::ParseError;

use super::*;
use crate::text::Text;
use crate::widgets::Widget;

mod edge_cases;
mod render;

#[fixture]
pub(super) fn scrollbar_no_arrows() -> Scrollbar<'static> {
	Scrollbar::new(ScrollbarOrientation::HorizontalTop)
		.begin_symbol(None)
		.end_symbol(None)
		.track_symbol(Some("-"))
		.thumb_symbol("#")
}

#[test]
fn scroll_direction_to_string() {
	assert_eq!(ScrollDirection::Forward.to_string(), "Forward");
	assert_eq!(ScrollDirection::Backward.to_string(), "Backward");
}

#[test]
fn scroll_direction_from_str() {
	assert_eq!("Forward".parse(), Ok(ScrollDirection::Forward));
	assert_eq!("Backward".parse(), Ok(ScrollDirection::Backward));
	assert_eq!(
		ScrollDirection::from_str(""),
		Err(ParseError::VariantNotFound)
	);
}

#[test]
fn scrollbar_orientation_to_string() {
	use ScrollbarOrientation::*;
	assert_eq!(VerticalRight.to_string(), "VerticalRight");
	assert_eq!(VerticalLeft.to_string(), "VerticalLeft");
	assert_eq!(HorizontalBottom.to_string(), "HorizontalBottom");
	assert_eq!(HorizontalTop.to_string(), "HorizontalTop");
}

#[test]
fn scrollbar_orientation_from_str() {
	use ScrollbarOrientation::*;
	assert_eq!("VerticalRight".parse(), Ok(VerticalRight));
	assert_eq!("VerticalLeft".parse(), Ok(VerticalLeft));
	assert_eq!("HorizontalBottom".parse(), Ok(HorizontalBottom));
	assert_eq!("HorizontalTop".parse(), Ok(HorizontalTop));
	assert_eq!(
		ScrollbarOrientation::from_str(""),
		Err(ParseError::VariantNotFound)
	);
}
