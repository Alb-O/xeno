use super::*;

#[test]
fn test_borders_debug() {
	assert_eq!(format!("{:?}", Borders::empty()), "NONE");
	assert_eq!(format!("{:?}", Borders::NONE), "NONE");
	assert_eq!(format!("{:?}", Borders::TOP), "TOP");
	assert_eq!(format!("{:?}", Borders::BOTTOM), "BOTTOM");
	assert_eq!(format!("{:?}", Borders::LEFT), "LEFT");
	assert_eq!(format!("{:?}", Borders::RIGHT), "RIGHT");
	assert_eq!(format!("{:?}", Borders::ALL), "ALL");
	assert_eq!(format!("{:?}", Borders::all()), "ALL");

	assert_eq!(
		format!("{:?}", Borders::TOP | Borders::BOTTOM),
		"TOP | BOTTOM"
	);
}

#[test]
fn can_be_const() {
	const NOTHING: Borders = border!();
	const JUST_TOP: Borders = border!(TOP);
	const TOP_BOTTOM: Borders = border!(TOP, BOTTOM);
	const RIGHT_OPEN: Borders = border!(TOP, LEFT, BOTTOM);

	assert_eq!(NOTHING, Borders::NONE);
	assert_eq!(JUST_TOP, Borders::TOP);
	assert_eq!(TOP_BOTTOM, Borders::TOP | Borders::BOTTOM);
	assert_eq!(RIGHT_OPEN, Borders::TOP | Borders::LEFT | Borders::BOTTOM);
}

#[test]
fn border_empty() {
	let empty = Borders::NONE;
	assert_eq!(empty, border!());
}

#[test]
fn border_all() {
	let all = Borders::ALL;
	assert_eq!(all, border!(ALL));
	assert_eq!(all, border!(TOP, BOTTOM, LEFT, RIGHT));
}

#[test]
fn border_left_right() {
	let left_right = Borders::from_bits(Borders::LEFT.bits() | Borders::RIGHT.bits());
	assert_eq!(left_right, Some(border!(RIGHT, LEFT)));
}
