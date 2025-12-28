//! Tests for Text type conversions - From impls, to_text, collect, etc.

use core::iter;

use rstest::rstest;

use super::*;

#[test]
fn from_string() {
	let text = Text::from(String::from("The first line\nThe second line"));
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn from_str() {
	let text = Text::from("The first line\nThe second line");
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn from_cow() {
	let text = Text::from(Cow::Borrowed("The first line\nThe second line"));
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn from_span() {
	let style = Style::new().yellow().italic();
	let text = Text::from(Span::styled("The first line\nThe second line", style));
	assert_eq!(
		text.lines,
		vec![Line::from(Span::styled(
			"The first line\nThe second line",
			style
		))]
	);
}

#[test]
fn from_line() {
	let text = Text::from(Line::from("The first line"));
	assert_eq!(text.lines, [Line::from("The first line")]);
}

#[rstest]
#[case(42, Text::from("42"))]
#[case("just\ntesting", Text::from("just\ntesting"))]
#[case(true, Text::from("true"))]
#[case(6.66, Text::from("6.66"))]
#[case('a', Text::from("a"))]
#[case(String::from("hello"), Text::from("hello"))]
#[case(-1, Text::from("-1"))]
#[case("line1\nline2", Text::from("line1\nline2"))]
#[case(
	"first line\nsecond line\nthird line",
	Text::from("first line\nsecond line\nthird line")
)]
#[case("trailing newline\n", Text::from("trailing newline\n"))]
fn to_text(#[case] value: impl fmt::Display, #[case] expected: Text) {
	assert_eq!(value.to_text(), expected);
}

#[test]
fn from_vec_line() {
	let text = Text::from(vec![
		Line::from("The first line"),
		Line::from("The second line"),
	]);
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn from_iterator() {
	let text = Text::from_iter(vec!["The first line", "The second line"]);
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn collect() {
	let text: Text = iter::once("The first line")
		.chain(iter::once("The second line"))
		.collect();
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn into_iter() {
	let text = Text::from("The first line\nThe second line");
	let mut iter = text.into_iter();
	assert_eq!(iter.next(), Some(Line::from("The first line")));
	assert_eq!(iter.next(), Some(Line::from("The second line")));
	assert_eq!(iter.next(), None);
}

#[rstest]
#[case::one_line("The first line")]
#[case::multiple_lines("The first line\nThe second line")]
fn display_raw_text(#[case] value: &str) {
	let text = Text::raw(value);
	assert_eq!(format!("{text}"), value);
}

#[test]
fn display_styled_text() {
	let styled_text = Text::styled(
		"The first line\nThe second line",
		Style::new().yellow().italic(),
	);

	assert_eq!(format!("{styled_text}"), "The first line\nThe second line");
}

#[test]
fn display_text_from_vec() {
	let text_from_vec = Text::from(vec![
		Line::from("The first line"),
		Line::from("The second line"),
	]);

	assert_eq!(
		format!("{text_from_vec}"),
		"The first line\nThe second line"
	);
}

#[test]
fn display_extended_text() {
	let mut text = Text::from("The first line\nThe second line");

	assert_eq!(format!("{text}"), "The first line\nThe second line");

	text.extend(vec![
		Line::from("The third line"),
		Line::from("The fourth line"),
	]);

	assert_eq!(
		format!("{text}"),
		"The first line\nThe second line\nThe third line\nThe fourth line"
	);
}
