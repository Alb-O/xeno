use std::borrow::Cow;

use pretty_assertions::assert_eq;

use super::*;
use crate::style::{Color, Modifier, Stylize};
use crate::text::{Line, Span};

#[test]
fn new_from_str() {
	let item = ListItem::new("Test item");
	assert_eq!(item.content, Text::from("Test item"));
	assert_eq!(item.style, Style::default());
}

#[test]
fn new_from_string() {
	let item = ListItem::new("Test item".to_string());
	assert_eq!(item.content, Text::from("Test item"));
	assert_eq!(item.style, Style::default());
}

#[test]
fn new_from_cow_str() {
	let item = ListItem::new(Cow::Borrowed("Test item"));
	assert_eq!(item.content, Text::from("Test item"));
	assert_eq!(item.style, Style::default());
}

#[test]
fn new_from_span() {
	let span = Span::styled("Test item", Style::default().fg(Color::Blue));
	let item = ListItem::new(span.clone());
	assert_eq!(item.content, Text::from(span));
	assert_eq!(item.style, Style::default());
}

#[test]
fn new_from_spans() {
	let spans = Line::from(vec![
		Span::styled("Test ", Style::default().fg(Color::Blue)),
		Span::styled("item", Style::default().fg(Color::Red)),
	]);
	let item = ListItem::new(spans.clone());
	assert_eq!(item.content, Text::from(spans));
	assert_eq!(item.style, Style::default());
}

#[test]
fn new_from_vec_spans() {
	let lines = vec![
		Line::from(vec![
			Span::styled("Test ", Style::default().fg(Color::Blue)),
			Span::styled("item", Style::default().fg(Color::Red)),
		]),
		Line::from(vec![
			Span::styled("Second ", Style::default().fg(Color::Green)),
			Span::styled("line", Style::default().fg(Color::Yellow)),
		]),
	];
	let item = ListItem::new(lines.clone());
	assert_eq!(item.content, Text::from(lines));
	assert_eq!(item.style, Style::default());
}

#[test]
fn str_into_list_item() {
	let s = "Test item";
	let item: ListItem = s.into();
	assert_eq!(item.content, Text::from(s));
	assert_eq!(item.style, Style::default());
}

#[test]
fn string_into_list_item() {
	let s = String::from("Test item");
	let item: ListItem = s.clone().into();
	assert_eq!(item.content, Text::from(s));
	assert_eq!(item.style, Style::default());
}

#[test]
fn span_into_list_item() {
	let s = Span::from("Test item");
	let item: ListItem = s.clone().into();
	assert_eq!(item.content, Text::from(s));
	assert_eq!(item.style, Style::default());
}

#[test]
fn vec_lines_into_list_item() {
	let lines = vec![Line::raw("l1"), Line::raw("l2")];
	let item: ListItem = lines.clone().into();
	assert_eq!(item.content, Text::from(lines));
	assert_eq!(item.style, Style::default());
}

#[test]
fn style() {
	let item = ListItem::new("Test item").style(Style::default().bg(Color::Red));
	assert_eq!(item.content, Text::from("Test item"));
	assert_eq!(item.style, Style::default().bg(Color::Red));
}

#[test]
fn height() {
	let item = ListItem::new("Test item");
	assert_eq!(item.height(), 1);

	let item = ListItem::new("Test item\nSecond line");
	assert_eq!(item.height(), 2);
}

#[test]
fn width() {
	let item = ListItem::new("Test item");
	assert_eq!(item.width(), 9);
}

#[test]
fn can_be_stylized() {
	assert_eq!(
		ListItem::new("").black().on_white().bold().not_dim().style,
		Style::default()
			.fg(Color::Black)
			.bg(Color::White)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::DIM)
	);
}
