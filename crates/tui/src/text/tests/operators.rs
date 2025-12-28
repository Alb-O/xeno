//! Tests for Text operator implementations - Add, AddAssign, Extend.

use super::*;

#[test]
fn add_line() {
	assert_eq!(
		Text::raw("Red").red() + Line::raw("Blue").blue(),
		Text {
			lines: vec![Line::raw("Red"), Line::raw("Blue").blue()],
			style: Style::new().red(),
			alignment: None,
		}
	);
}

#[test]
fn add_text() {
	assert_eq!(
		Text::raw("Red").red() + Text::raw("Blue").blue(),
		Text {
			lines: vec![Line::raw("Red"), Line::raw("Blue")],
			style: Style::new().red(),
			alignment: None,
		}
	);
}

#[test]
fn add_assign_text() {
	let mut text = Text::raw("Red").red();
	text += Text::raw("Blue").blue();
	assert_eq!(
		text,
		Text {
			lines: vec![Line::raw("Red"), Line::raw("Blue")],
			style: Style::new().red(),
			alignment: None,
		}
	);
}

#[test]
fn add_assign_line() {
	let mut text = Text::raw("Red").red();
	text += Line::raw("Blue").blue();
	assert_eq!(
		text,
		Text {
			lines: vec![Line::raw("Red"), Line::raw("Blue").blue()],
			style: Style::new().red(),
			alignment: None,
		}
	);
}

#[test]
fn extend() {
	let mut text = Text::from("The first line\nThe second line");
	text.extend(vec![
		Line::from("The third line"),
		Line::from("The fourth line"),
	]);
	assert_eq!(
		text.lines,
		vec![
			Line::from("The first line"),
			Line::from("The second line"),
			Line::from("The third line"),
			Line::from("The fourth line"),
		]
	);
}

#[test]
fn extend_from_iter() {
	let mut text = Text::from("The first line\nThe second line");
	text.extend(vec![
		Line::from("The third line"),
		Line::from("The fourth line"),
	]);
	assert_eq!(
		text.lines,
		vec![
			Line::from("The first line"),
			Line::from("The second line"),
			Line::from("The third line"),
			Line::from("The fourth line"),
		]
	);
}

#[test]
fn extend_from_iter_str() {
	let mut text = Text::from("The first line\nThe second line");
	text.extend(vec!["The third line", "The fourth line"]);
	assert_eq!(
		text.lines,
		vec![
			Line::from("The first line"),
			Line::from("The second line"),
			Line::from("The third line"),
			Line::from("The fourth line"),
		]
	);
}
