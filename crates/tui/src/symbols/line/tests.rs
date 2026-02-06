use indoc::{formatdoc, indoc};

use super::*;

#[test]
fn default() {
	assert_eq!(Set::default(), NORMAL);
}

/// A helper function to render a set of symbols.
fn render(set: Set) -> String {
	formatdoc!(
		"{}{}{}{}
         {}{}{}{}
         {}{}{}{}
         {}{}{}{}",
		set.top_left,
		set.horizontal,
		set.horizontal_down,
		set.top_right,
		set.vertical,
		" ",
		set.vertical,
		set.vertical,
		set.vertical_right,
		set.horizontal,
		set.cross,
		set.vertical_left,
		set.bottom_left,
		set.horizontal,
		set.horizontal_up,
		set.bottom_right
	)
}

#[test]
fn normal() {
	assert_eq!(
		render(NORMAL),
		indoc!(
			"┌─┬┐
             │ ││
             ├─┼┤
             └─┴┘"
		)
	);
}

#[test]
fn rounded() {
	assert_eq!(
		render(ROUNDED),
		indoc!(
			"╭─┬╮
             │ ││
             ├─┼┤
             ╰─┴╯"
		)
	);
}

#[test]
fn double() {
	assert_eq!(
		render(DOUBLE),
		indoc!(
			"╔═╦╗
             ║ ║║
             ╠═╬╣
             ╚═╩╝"
		)
	);
}

#[test]
fn thick() {
	assert_eq!(
		render(THICK),
		indoc!(
			"┏━┳┓
             ┃ ┃┃
             ┣━╋┫
             ┗━┻┛"
		)
	);
}
