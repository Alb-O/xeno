//! Tests for Text widget rendering.

use rstest::rstest;

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
fn render_out_of_bounds(#[from(super::small_buf)] mut small_buf: Buffer) {
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
