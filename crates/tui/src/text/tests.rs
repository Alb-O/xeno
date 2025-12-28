use alloc::format;
use core::iter;

use rstest::{fixture, rstest};

use super::*;
use crate::style::{Color, Modifier, Stylize};

#[fixture]
fn small_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 1))
}

#[test]
fn raw() {
	let text = Text::raw("The first line\nThe second line");
	assert_eq!(
		text.lines,
		vec![Line::from("The first line"), Line::from("The second line")]
	);
}

#[test]
fn styled() {
	let style = Style::new().yellow().italic();
	let styled_text = Text::styled("The first line\nThe second line", style);

	let mut text = Text::raw("The first line\nThe second line");
	text.style = style;

	assert_eq!(styled_text, text);
}

#[test]
fn width() {
	let text = Text::from("The first line\nThe second line");
	assert_eq!(15, text.width());
}

#[test]
fn height() {
	let text = Text::from("The first line\nThe second line");
	assert_eq!(2, text.height());
}

#[test]
fn patch_style() {
	let style = Style::new().yellow().italic();
	let style2 = Style::new().red().underlined();
	let text = Text::styled("The first line\nThe second line", style).patch_style(style2);

	let expected_style = Style::new().red().italic().underlined();
	let expected_text = Text::styled("The first line\nThe second line", expected_style);

	assert_eq!(text, expected_text);
}

#[test]
fn reset_style() {
	let style = Style::new().yellow().italic();
	let text = Text::styled("The first line\nThe second line", style).reset_style();

	assert_eq!(text.style, Style::reset());
}

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

#[test]
fn stylize() {
	assert_eq!(Text::default().green().style, Color::Green.into());
	assert_eq!(
		Text::default().on_green().style,
		Style::new().bg(Color::Green)
	);
	assert_eq!(Text::default().italic().style, Modifier::ITALIC.into());
}

#[test]
fn left_aligned() {
	let text = Text::from("Hello, world!").left_aligned();
	assert_eq!(text.alignment, Some(Alignment::Left));
}

#[test]
fn centered() {
	let text = Text::from("Hello, world!").centered();
	assert_eq!(text.alignment, Some(Alignment::Center));
}

#[test]
fn right_aligned() {
	let text = Text::from("Hello, world!").right_aligned();
	assert_eq!(text.alignment, Some(Alignment::Right));
}

#[test]
fn push_line() {
	let mut text = Text::from("A");
	text.push_line(Line::from("B"));
	text.push_line(Span::from("C"));
	text.push_line("D");
	assert_eq!(
		text.lines,
		vec![
			Line::raw("A"),
			Line::raw("B"),
			Line::raw("C"),
			Line::raw("D")
		]
	);
}

#[test]
fn push_line_empty() {
	let mut text = Text::default();
	text.push_line(Line::from("Hello, world!"));
	assert_eq!(text.lines, [Line::from("Hello, world!")]);
}

#[test]
fn push_span() {
	let mut text = Text::from("A");
	text.push_span(Span::raw("B"));
	text.push_span("C");
	assert_eq!(
		text.lines,
		vec![Line::from(vec![
			Span::raw("A"),
			Span::raw("B"),
			Span::raw("C")
		])],
	);
}

#[test]
fn push_span_empty() {
	let mut text = Text::default();
	text.push_span(Span::raw("Hello, world!"));
	assert_eq!(text.lines, [Line::from(Span::raw("Hello, world!"))]);
}

mod widget {
	use super::*;

	#[test]
	fn render() {
		let text = Text::from("foo");
		let area = Rect::new(0, 0, 5, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["foo  "]));
	}

	#[rstest]
	fn render_out_of_bounds(mut small_buf: Buffer) {
		let out_of_bounds_area = Rect::new(20, 20, 10, 1);
		Text::from("Hello, world!").render(out_of_bounds_area, &mut small_buf);
		assert_eq!(small_buf, Buffer::empty(small_buf.area));
	}

	#[test]
	fn render_right_aligned() {
		let text = Text::from("foo").alignment(Alignment::Right);
		let area = Rect::new(0, 0, 5, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["  foo"]));
	}

	#[test]
	fn render_centered_odd() {
		let text = Text::from("foo").alignment(Alignment::Center);
		let area = Rect::new(0, 0, 5, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([" foo "]));
	}

	#[test]
	fn render_centered_even() {
		let text = Text::from("foo").alignment(Alignment::Center);
		let area = Rect::new(0, 0, 6, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines([" foo  "]));
	}

	#[test]
	fn render_right_aligned_with_truncation() {
		let text = Text::from("123456789").alignment(Alignment::Right);
		let area = Rect::new(0, 0, 5, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["56789"]));
	}

	#[test]
	fn render_centered_odd_with_truncation() {
		let text = Text::from("123456789").alignment(Alignment::Center);
		let area = Rect::new(0, 0, 5, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["34567"]));
	}

	#[test]
	fn render_centered_even_with_truncation() {
		let text = Text::from("123456789").alignment(Alignment::Center);
		let area = Rect::new(0, 0, 6, 1);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["234567"]));
	}

	#[test]
	fn render_one_line_right() {
		let text = Text::from(vec![
			"foo".into(),
			Line::from("bar").alignment(Alignment::Center),
		])
		.alignment(Alignment::Right);
		let area = Rect::new(0, 0, 5, 2);
		let mut buf = Buffer::empty(area);
		text.render(area, &mut buf);
		assert_eq!(buf, Buffer::with_lines(["  foo", " bar "]));
	}

	#[test]
	fn render_only_styles_line_area() {
		let area = Rect::new(0, 0, 5, 1);
		let mut buf = Buffer::empty(area);
		Text::from("foo".on_blue()).render(area, &mut buf);

		let mut expected = Buffer::with_lines(["foo  "]);
		expected.set_style(Rect::new(0, 0, 3, 1), Style::new().bg(Color::Blue));
		assert_eq!(buf, expected);
	}

	#[test]
	fn render_truncates() {
		let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
		Text::from("foobar".on_blue()).render(Rect::new(0, 0, 3, 1), &mut buf);

		let mut expected = Buffer::with_lines(["foo   "]);
		expected.set_style(Rect::new(0, 0, 3, 1), Style::new().bg(Color::Blue));
		assert_eq!(buf, expected);
	}
}

mod iterators {
	use super::*;

	/// a fixture used in the tests below to avoid repeating the same setup
	#[fixture]
	fn hello_world() -> Text<'static> {
		Text::from(vec![
			Line::styled("Hello ", Color::Blue),
			Line::styled("world!", Color::Green),
		])
	}

	#[rstest]
	fn iter(hello_world: Text<'_>) {
		let mut iter = hello_world.iter();
		assert_eq!(iter.next(), Some(&Line::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&Line::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn iter_mut(mut hello_world: Text<'_>) {
		let mut iter = hello_world.iter_mut();
		assert_eq!(iter.next(), Some(&mut Line::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&mut Line::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn into_iter(hello_world: Text<'_>) {
		let mut iter = hello_world.into_iter();
		assert_eq!(iter.next(), Some(Line::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(Line::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn into_iter_ref(hello_world: Text<'_>) {
		let mut iter = (&hello_world).into_iter();
		assert_eq!(iter.next(), Some(&Line::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&Line::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[test]
	fn into_iter_mut_ref() {
		let mut hello_world = Text::from(vec![
			Line::styled("Hello ", Color::Blue),
			Line::styled("world!", Color::Green),
		]);
		let mut iter = (&mut hello_world).into_iter();
		assert_eq!(iter.next(), Some(&mut Line::styled("Hello ", Color::Blue)));
		assert_eq!(iter.next(), Some(&mut Line::styled("world!", Color::Green)));
		assert_eq!(iter.next(), None);
	}

	#[rstest]
	fn for_loop_ref(hello_world: Text<'_>) {
		let mut result = String::new();
		for line in &hello_world {
			result.push_str(line.to_string().as_ref());
		}
		assert_eq!(result, "Hello world!");
	}

	#[rstest]
	fn for_loop_mut_ref() {
		let mut hello_world = Text::from(vec![
			Line::styled("Hello ", Color::Blue),
			Line::styled("world!", Color::Green),
		]);
		let mut result = String::new();
		for line in &mut hello_world {
			result.push_str(line.to_string().as_ref());
		}
		assert_eq!(result, "Hello world!");
	}

	#[rstest]
	fn for_loop_into(hello_world: Text<'_>) {
		let mut result = String::new();
		for line in hello_world {
			result.push_str(line.to_string().as_ref());
		}
		assert_eq!(result, "Hello world!");
	}
}

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
