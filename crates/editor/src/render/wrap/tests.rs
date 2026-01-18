use super::*;

fn wrap(text: &str, width: usize) -> Vec<String> {
	wrap_line(text, width, 4)
		.into_iter()
		.map(|s| s.text)
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
