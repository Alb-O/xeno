use core::iter;

use rstest::{fixture, rstest};

use super::*;
use crate::style::{Color, Modifier, Stylize};

mod iterators;
mod widget;

#[fixture]
pub(super) fn small_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 1))
}

#[test]
fn raw_str() {
	let line = Line::raw("test content");
	assert_eq!(line.spans, [Span::raw("test content")]);
	assert_eq!(line.alignment, None);

	let line = Line::raw("a\nb");
	assert_eq!(line.spans, [Span::raw("a"), Span::raw("b")]);
	assert_eq!(line.alignment, None);
}

#[test]
fn styled_str() {
	let style = Style::new().yellow();
	let content = "Hello, world!";
	let line = Line::styled(content, style);
	assert_eq!(line.spans, [Span::raw(content)]);
	assert_eq!(line.style, style);
}

#[test]
fn styled_string() {
	let style = Style::new().yellow();
	let content = String::from("Hello, world!");
	let line = Line::styled(content.clone(), style);
	assert_eq!(line.spans, [Span::raw(content)]);
	assert_eq!(line.style, style);
}

#[test]
fn styled_cow() {
	let style = Style::new().yellow();
	let content = Cow::from("Hello, world!");
	let line = Line::styled(content.clone(), style);
	assert_eq!(line.spans, [Span::raw(content)]);
	assert_eq!(line.style, style);
}

#[test]
fn spans_vec() {
	let line = Line::default().spans(vec!["Hello".blue(), " world!".green()]);
	assert_eq!(
		line.spans,
		vec![Span::styled("Hello", Style::new().blue()), Span::styled(" world!", Style::new().green()),]
	);
}

#[test]
fn spans_iter() {
	let line = Line::default().spans([1, 2, 3].iter().map(|i| format!("Item {i}")));
	assert_eq!(line.spans, vec![Span::raw("Item 1"), Span::raw("Item 2"), Span::raw("Item 3"),]);
}

#[test]
fn style() {
	let line = Line::default().style(Style::new().red());
	assert_eq!(line.style, Style::new().red());
}

#[test]
fn alignment() {
	let line = Line::from("This is left").alignment(HorizontalAlignment::Left);
	assert_eq!(Some(HorizontalAlignment::Left), line.alignment);

	let line = Line::from("This is default");
	assert_eq!(None, line.alignment);
}

#[test]
fn width() {
	let line = Line::from(vec![Span::styled("My", Style::default().fg(Color::Yellow)), Span::raw(" text")]);
	assert_eq!(7, line.width());

	let empty_line = Line::default();
	assert_eq!(0, empty_line.width());
}

#[test]
fn patch_style() {
	let raw_line = Line::styled("foobar", Color::Yellow);
	let styled_line = Line::styled("foobar", (Color::Yellow, Modifier::ITALIC));

	assert_ne!(raw_line, styled_line);

	let raw_line = raw_line.patch_style(Modifier::ITALIC);
	assert_eq!(raw_line, styled_line);
}

#[test]
fn reset_style() {
	let line = Line::styled("foobar", Style::default().yellow().on_red().italic()).reset_style();

	assert_eq!(Style::reset(), line.style);
}

#[test]
fn stylize() {
	assert_eq!(Line::default().green().style, Color::Green.into());
	assert_eq!(Line::default().on_green().style, Style::new().bg(Color::Green));
	assert_eq!(Line::default().italic().style, Modifier::ITALIC.into());
}

#[test]
fn from_string() {
	let s = String::from("Hello, world!");
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello, world!")]);

	let s = String::from("Hello\nworld!");
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello"), Span::from("world!")]);
}

#[test]
fn from_str() {
	let s = "Hello, world!";
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello, world!")]);

	let s = "Hello\nworld!";
	let line = Line::from(s);
	assert_eq!(line.spans, [Span::from("Hello"), Span::from("world!")]);
}

#[test]
fn to_line() {
	let line = 42.to_line();
	assert_eq!(line.spans, [Span::from("42")]);
}

#[test]
fn from_vec() {
	let spans = vec![
		Span::styled("Hello,", Style::default().fg(Color::Red)),
		Span::styled(" world!", Style::default().fg(Color::Green)),
	];
	let line = Line::from(spans.clone());
	assert_eq!(line.spans, spans);
}

#[test]
fn from_iter() {
	let line = Line::from_iter(vec!["Hello".blue(), " world!".green()]);
	assert_eq!(
		line.spans,
		vec![Span::styled("Hello", Style::new().blue()), Span::styled(" world!", Style::new().green()),]
	);
}

#[test]
fn collect() {
	let line: Line = iter::once("Hello".blue()).chain(iter::once(" world!".green())).collect();
	assert_eq!(
		line.spans,
		vec![Span::styled("Hello", Style::new().blue()), Span::styled(" world!", Style::new().green()),]
	);
}

#[test]
fn from_span() {
	let span = Span::styled("Hello, world!", Style::default().fg(Color::Yellow));
	let line = Line::from(span.clone());
	assert_eq!(line.spans, [span]);
}

#[test]
fn add_span() {
	assert_eq!(
		Line::raw("Red").red() + Span::raw("blue").blue(),
		Line {
			spans: vec![Span::raw("Red"), Span::raw("blue").blue()],
			style: Style::new().red(),
			alignment: None,
		},
	);
}

#[test]
fn add_line() {
	assert_eq!(
		Line::raw("Red").red() + Line::raw("Blue").blue(),
		Text {
			lines: vec![Line::raw("Red").red(), Line::raw("Blue").blue()],
			style: Style::default(),
			alignment: None,
		}
	);
}

#[test]
fn add_assign_span() {
	let mut line = Line::raw("Red").red();
	line += Span::raw("Blue").blue();
	assert_eq!(
		line,
		Line {
			spans: vec![Span::raw("Red"), Span::raw("Blue").blue()],
			style: Style::new().red(),
			alignment: None,
		},
	);
}

#[test]
fn extend() {
	let mut line = Line::from("Hello, ");
	line.extend([Span::raw("world!")]);
	assert_eq!(line.spans, [Span::raw("Hello, "), Span::raw("world!")]);

	let mut line = Line::from("Hello, ");
	line.extend([Span::raw("world! "), Span::raw("How are you?")]);
	assert_eq!(line.spans, [Span::raw("Hello, "), Span::raw("world! "), Span::raw("How are you?")]);
}

#[test]
fn into_string() {
	let line = Line::from(vec![
		Span::styled("Hello,", Style::default().fg(Color::Red)),
		Span::styled(" world!", Style::default().fg(Color::Green)),
	]);
	let s: String = line.into();
	assert_eq!(s, "Hello, world!");
}

#[test]
fn styled_graphemes() {
	const RED: Style = Style::new().red();
	const GREEN: Style = Style::new().green();
	const BLUE: Style = Style::new().blue();
	const RED_ON_WHITE: Style = Style::new().red().on_white();
	const GREEN_ON_WHITE: Style = Style::new().green().on_white();
	const BLUE_ON_WHITE: Style = Style::new().blue().on_white();

	let line = Line::from(vec![Span::styled("He", RED), Span::styled("ll", GREEN), Span::styled("o!", BLUE)]);
	let styled_graphemes = line.styled_graphemes(Style::new().bg(Color::White)).collect::<Vec<StyledGrapheme>>();
	assert_eq!(
		styled_graphemes,
		vec![
			StyledGrapheme::new("H", RED_ON_WHITE),
			StyledGrapheme::new("e", RED_ON_WHITE),
			StyledGrapheme::new("l", GREEN_ON_WHITE),
			StyledGrapheme::new("l", GREEN_ON_WHITE),
			StyledGrapheme::new("o", BLUE_ON_WHITE),
			StyledGrapheme::new("!", BLUE_ON_WHITE),
		],
	);
}

#[test]
fn display_line_from_vec() {
	let line_from_vec = Line::from(vec![Span::raw("Hello,"), Span::raw(" world!")]);

	assert_eq!(format!("{line_from_vec}"), "Hello, world!");
}

#[test]
fn display_styled_line() {
	let styled_line = Line::styled("Hello, world!", Style::new().green().italic());

	assert_eq!(format!("{styled_line}"), "Hello, world!");
}

#[test]
fn display_line_from_styled_span() {
	let styled_span = Span::styled("Hello, world!", Style::new().green().italic());
	let line_from_styled_span = Line::from(styled_span);

	assert_eq!(format!("{line_from_styled_span}"), "Hello, world!");
}

#[test]
fn left_aligned() {
	let line = Line::from("Hello, world!").left_aligned();
	assert_eq!(line.alignment, Some(HorizontalAlignment::Left));
}

#[test]
fn centered() {
	let line = Line::from("Hello, world!").centered();
	assert_eq!(line.alignment, Some(HorizontalAlignment::Center));
}

#[test]
fn right_aligned() {
	let line = Line::from("Hello, world!").right_aligned();
	assert_eq!(line.alignment, Some(HorizontalAlignment::Right));
}

#[test]
pub fn push_span() {
	let mut line = Line::from("A");
	line.push_span(Span::raw("B"));
	line.push_span("C");
	assert_eq!(line.spans, vec![Span::raw("A"), Span::raw("B"), Span::raw("C")]);
}

#[rstest]
#[case::empty(Line::default(), "Line::default()")]
#[case::raw(Line::raw("Hello, world!"), r#"Line::from("Hello, world!")"#)]
#[case::styled(Line::styled("Hello, world!", Color::Yellow), r#"Line::from("Hello, world!").yellow()"#)]
#[case::styled_complex(
        Line::from(String::from("Hello, world!")).green().on_blue().bold().italic().not_dim(),
        r#"Line::from("Hello, world!").green().on_blue().bold().italic().not_dim()"#
    )]
#[case::styled_span(Line::from(Span::styled("Hello, world!", Color::Yellow)), r#"Line::from(Span::from("Hello, world!").yellow())"#)]
#[case::styled_line_and_span(
        Line::from(vec![
            Span::styled("Hello", Color::Yellow),
            Span::styled(" world!", Color::Green),
        ]).italic(),
        r#"Line::from_iter([Span::from("Hello").yellow(), Span::from(" world!").green()]).italic()"#
    )]
#[case::spans_vec(
        Line::from(vec![
            Span::styled("Hello", Color::Blue),
            Span::styled(" world!", Color::Green),
        ]),
        r#"Line::from_iter([Span::from("Hello").blue(), Span::from(" world!").green()])"#,
    )]
#[case::left_aligned(
        Line::from("Hello, world!").left_aligned(),
        r#"Line::from("Hello, world!").left_aligned()"#
    )]
#[case::centered(
        Line::from("Hello, world!").centered(),
        r#"Line::from("Hello, world!").centered()"#
    )]
#[case::right_aligned(
        Line::from("Hello, world!").right_aligned(),
        r#"Line::from("Hello, world!").right_aligned()"#
    )]
fn debug(#[case] line: Line, #[case] expected: &str) {
	assert_eq!(format!("{line:?}"), expected);
}
