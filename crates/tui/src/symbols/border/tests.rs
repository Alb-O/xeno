//! Tests for border symbol sets.

use alloc::format;
use alloc::string::String;

use indoc::formatdoc;

use super::*;

mod sets;

/// A helper function to render a border set to a string.
///
/// '░' (U+2591 Light Shade) is used as a placeholder for empty space to make it easier to see
/// the size of the border symbols.
pub(super) fn render(set: Set) -> String {
	formatdoc!(
		"░░░░░░
         ░{}{}{}{}░
         ░{}░░{}░
         ░{}░░{}░
         ░{}{}{}{}░
         ░░░░░░",
		set.top_left,
		set.horizontal_top,
		set.horizontal_top,
		set.top_right,
		set.vertical_left,
		set.vertical_right,
		set.vertical_left,
		set.vertical_right,
		set.bottom_left,
		set.horizontal_bottom,
		set.horizontal_bottom,
		set.bottom_right
	)
}

#[test]
fn default() {
	assert_eq!(Set::default(), PLAIN);
}

#[test]
fn border_set_from_line_set() {
	let custom_line_set = line::Set {
		top_left: "a",
		top_right: "b",
		bottom_left: "c",
		bottom_right: "d",
		vertical: "e",
		horizontal: "f",
		vertical_left: "g",
		vertical_right: "h",
		horizontal_down: "i",
		horizontal_up: "j",
		cross: "k",
	};

	let border_set = from_line_set(custom_line_set);

	assert_eq!(
		border_set,
		Set {
			top_left: "a",
			top_right: "b",
			bottom_left: "c",
			bottom_right: "d",
			vertical_left: "e",
			vertical_right: "e",
			horizontal_bottom: "f",
			horizontal_top: "f",
		}
	);
}
