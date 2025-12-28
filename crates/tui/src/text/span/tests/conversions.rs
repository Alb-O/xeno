//! Span conversion and display tests

use alloc::format;
use alloc::string::String;

use super::*;

#[test]
fn from_ref_str_borrowed_cow() {
	let content = "test content";
	let span = Span::from(content);
	assert_eq!(span.content, Cow::Borrowed(content));
	assert_eq!(span.style, Style::default());
}

#[test]
fn from_string_ref_str_borrowed_cow() {
	let content = String::from("test content");
	let span = Span::from(content.as_str());
	assert_eq!(span.content, Cow::Borrowed(content.as_str()));
	assert_eq!(span.style, Style::default());
}

#[test]
fn from_string_owned_cow() {
	let content = String::from("test content");
	let span = Span::from(content.clone());
	assert_eq!(span.content, Cow::Owned::<str>(content));
	assert_eq!(span.style, Style::default());
}

#[test]
fn from_ref_string_borrowed_cow() {
	let content = String::from("test content");
	let span = Span::from(&content);
	assert_eq!(span.content, Cow::Borrowed(content.as_str()));
	assert_eq!(span.style, Style::default());
}

#[test]
fn to_span() {
	assert_eq!(42.to_span(), Span::raw("42"));
	assert_eq!("test".to_span(), Span::raw("test"));
}

#[test]
fn display_span() {
	let span = Span::raw("test content");
	assert_eq!(format!("{span}"), "test content");
	assert_eq!(format!("{span:.4}"), "test");
}

#[test]
fn display_newline_span() {
	let span = Span::raw("test\ncontent");
	assert_eq!(format!("{span}"), "testcontent");
}

#[test]
fn display_styled_span() {
	let stylized_span = Span::styled("stylized test content", Style::new().green());
	assert_eq!(format!("{stylized_span}"), "stylized test content");
	assert_eq!(format!("{stylized_span:.8}"), "stylized");
}
