//! Tests for Text Debug implementation.

use rstest::rstest;

use super::*;

#[rstest]
#[case::default(Text::default(), "Text::default()")]
// TODO jm: these could be improved to inspect the line / span if there's only one. e.g.
// Text::from("Hello, world!") and Text::from("Hello, world!".blue()) but the current
// implementation is good enough for now.
#[case::raw(
	Text::raw("Hello, world!"),
	r#"Text::from(Line::from("Hello, world!"))"#
)]
#[case::styled(
	Text::styled("Hello, world!", Color::Yellow),
	r#"Text::from(Line::from("Hello, world!")).yellow()"#
)]
#[case::complex_styled(
    Text::from("Hello, world!").yellow().on_blue().bold().italic().not_dim().not_hidden(),
    r#"Text::from(Line::from("Hello, world!")).yellow().on_blue().bold().italic().not_dim().not_hidden()"#
)]
#[case::alignment(
    Text::from("Hello, world!").centered(),
    r#"Text::from(Line::from("Hello, world!")).centered()"#
)]
#[case::styled_alignment(
    Text::styled("Hello, world!", Color::Yellow).centered(),
    r#"Text::from(Line::from("Hello, world!")).yellow().centered()"#
)]
#[case::multiple_lines(
    Text::from(vec![
        Line::from("Hello, world!"),
        Line::from("How are you?")
    ]),
    r#"Text::from_iter([Line::from("Hello, world!"), Line::from("How are you?")])"#
)]
fn debug(#[case] text: Text, #[case] expected: &str) {
	assert_eq!(format!("{text:?}"), expected);
}

#[test]
fn debug_alternate() {
	let text = Text::from_iter([
		Line::from("Hello, world!"),
		Line::from("How are you?").bold().left_aligned(),
		Line::from_iter([
			Span::from("I'm "),
			Span::from("doing ").italic(),
			Span::from("great!").bold(),
		]),
	])
	.on_blue()
	.italic()
	.centered();
	assert_eq!(
		format!("{text:#?}"),
		indoc::indoc! {r#"
            Text::from_iter([
                Line::from("Hello, world!"),
                Line::from("How are you?").bold().left_aligned(),
                Line::from_iter([
                    Span::from("I'm "),
                    Span::from("doing ").italic(),
                    Span::from("great!").bold(),
                ]),
            ]).on_blue().italic().centered()"#}
	);
}
