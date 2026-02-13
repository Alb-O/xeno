use ropey::Rope;

use super::*;

#[test]
fn test_no_trailing_newline() {
	let text = Rope::from("hello\nworld");
	assert_eq!(text.len_lines(), 2);
	assert_eq!(visible_line_count(text.slice(..)), 2);
}

#[test]
fn test_trailing_newline() {
	let text = Rope::from("hello\nworld\n");
	assert_eq!(text.len_lines(), 3);
	assert_eq!(visible_line_count(text.slice(..)), 3);
}

#[test]
fn test_single_line_no_newline() {
	let text = Rope::from("hello");
	assert_eq!(text.len_lines(), 1);
	assert_eq!(visible_line_count(text.slice(..)), 1);
}

#[test]
fn test_single_line_with_newline() {
	let text = Rope::from("hello\n");
	assert_eq!(text.len_lines(), 2);
	assert_eq!(visible_line_count(text.slice(..)), 2);
}

#[test]
fn test_empty() {
	let text = Rope::from("");
	assert_eq!(text.len_lines(), 1);
	assert_eq!(visible_line_count(text.slice(..)), 1);
}

#[test]
fn test_only_newline() {
	let text = Rope::from("\n");
	assert_eq!(text.len_lines(), 2);
	assert_eq!(visible_line_count(text.slice(..)), 2);
}
