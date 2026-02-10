use super::*;

fn wrap(text: &str, width: usize) -> Vec<String> {
	wrap_line(text, width, 4)
		.into_iter()
		.map(|s| s.text)
		.collect()
}

fn wrap_with_indent(text: &str, width: usize) -> Vec<(String, usize)> {
	wrap_line(text, width, 4)
		.into_iter()
		.map(|s| (s.text, s.indent_cols))
		.collect()
}

#[test]
fn basic_words() {
	assert_eq!(wrap("hello world", 6), vec!["hello ", "world"]);
}

#[test]
fn trailing_period_stays_with_word() {
	assert_eq!(wrap("hello. world", 7), vec!["hello. ", "world"]);
}

#[test]
fn trailing_comma_stays_with_word() {
	assert_eq!(wrap("hello, world", 7), vec!["hello, ", "world"]);
}

#[test]
fn closing_paren_stays_with_word() {
	assert_eq!(wrap("(hello) world", 8), vec!["(hello) ", "world"]);
}

#[test]
fn opening_paren_stays_with_word() {
	assert_eq!(wrap("call (foo)", 6), vec!["call ", "(foo)"]);
}

#[test]
fn path_separator_breakable() {
	assert_eq!(wrap("foo-bar-baz", 8), vec!["foo-bar-", "baz"]);
	assert_eq!(wrap("path/to/file", 8), vec!["path/to/", "file"]);
}

#[test]
fn multiple_trailing_punct() {
	assert_eq!(wrap("end.) next", 6), vec!["end.) ", "next"]);
}

#[test]
fn quote_with_word() {
	assert_eq!(wrap("say \"hi\" ok", 9), vec!["say \"hi\" ", "ok"]);
}

#[test]
fn indent_aware_wrapping() {
	// Indented line at sufficient width: continuation segments have indent_cols set
	// "    hello world" at width 30 (enough room for 20+ chars after 4-char indent)
	let result = wrap_with_indent("    hello world this is a test of wrapping", 30);
	assert!(result.len() >= 2);
	assert_eq!(result[0].1, 0); // first segment has no indent
	assert_eq!(result[1].1, 4); // continuation has 4-space indent
}

#[test]
fn no_indent_no_continuation_indent() {
	// Non-indented line: continuation should have 0 indent
	let result = wrap_with_indent("hello world foo bar baz qux", 15);
	assert!(result.len() >= 2);
	assert_eq!(result[0].1, 0);
	assert_eq!(result[1].1, 0); // no leading indent means no continuation indent
}

#[test]
fn tab_indent_aware() {
	// Tab-indented line (tab_width=4) at sufficient width
	let result = wrap_with_indent("\thello world this is a longer line", 30);
	assert!(result.len() >= 2);
	assert_eq!(result[0].1, 0); // first has no indent padding
	assert_eq!(result[1].1, 4); // continuation gets tab's visual width
}

#[test]
fn narrow_window_disables_indent() {
	// When window is too narrow (< 20 chars after indent), indent-aware wrapping is disabled
	// 4-space indent with width 20 leaves only 16 chars, below the 20-char minimum
	let result = wrap_with_indent("    hello world foo bar", 20);
	assert!(result.len() >= 2);
	assert_eq!(result[0].1, 0);
	assert_eq!(result[1].1, 0); // indent disabled due to narrow width
}

#[test]
fn deep_indent_disables_indent() {
	// Deep indent that would leave too little content space
	let result = wrap_with_indent("                deep indent text here", 30);
	assert!(result.len() >= 2);
	// 16-space indent with width 30 leaves only 14 chars, below minimum
	assert_eq!(result[1].1, 0); // indent disabled
}

#[test]
fn wrap_line_ranges_rope_wide_char_progress() {
	use xeno_primitives::Rope;
	let rope = Rope::from("🚀🚀🚀");
	let segments = wrap_line_ranges_rope(rope.slice(..), 1, 4);
	// Should not infinite loop and should have 3 segments of 1 char each
	assert_eq!(segments.len(), 3);
	for seg in segments {
		assert_eq!(seg.char_len, 1);
	}
}

#[test]
fn control_chars_have_single_cell_width() {
	assert_eq!(cell_width('\x1b', 0, 4), 1);
	assert_eq!(cell_width('\r', 0, 4), 1);
}
