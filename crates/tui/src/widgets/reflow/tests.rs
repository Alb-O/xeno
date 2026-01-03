use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use unicode_segmentation::UnicodeSegmentation;

use super::*;
use crate::style::Style;
use crate::text::{Line, Text};

#[derive(Clone, Copy)]
enum Composer {
	WordWrapper { trim: bool },
	LineTruncator,
}

fn run_composer<'a>(
	which: Composer,
	text: impl Into<Text<'a>>,
	text_area_width: u16,
) -> (Vec<String>, Vec<u16>, Vec<HorizontalAlignment>) {
	let text = text.into();
	let styled_lines = text.iter().map(|line| {
		(
			line.iter()
				.flat_map(|span| span.styled_graphemes(Style::default())),
			line.alignment.unwrap_or(HorizontalAlignment::Left),
		)
	});

	let mut composer: Box<dyn LineComposer> = match which {
		Composer::WordWrapper { trim } => {
			Box::new(WordWrapper::new(styled_lines, text_area_width, trim))
		}
		Composer::LineTruncator => Box::new(LineTruncator::new(styled_lines, text_area_width)),
	};
	let mut lines = vec![];
	let mut widths = vec![];
	let mut alignments = vec![];
	while let Some(WrappedLine {
		graphemes,
		width,
		alignment,
	}) = composer.next_line()
	{
		let line = graphemes
			.iter()
			.map(|StyledGrapheme { symbol, .. }| *symbol)
			.collect::<String>();
		assert!(width <= text_area_width);
		lines.push(line);
		widths.push(width);
		alignments.push(alignment);
	}
	(lines, widths, alignments)
}

#[test]
fn line_composer_one_line() {
	let width = 40;
	for i in 1..width {
		let text = "a".repeat(i);
		let (word_wrapper, _, _) =
			run_composer(Composer::WordWrapper { trim: true }, &*text, width as u16);
		let (line_truncator, _, _) = run_composer(Composer::LineTruncator, &*text, width as u16);
		let expected = vec![text];
		assert_eq!(word_wrapper, expected);
		assert_eq!(line_truncator, expected);
	}
}

#[test]
fn line_composer_short_lines() {
	let width = 20;
	let text = "abcdefg\nhijklmno\npabcdefg\nhijklmn\nopabcdefghijk\nlmnopabcd\n\n\nefghijklmno";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);

	let wrapped: Vec<&str> = text.split('\n').collect();
	assert_eq!(word_wrapper, wrapped);
	assert_eq!(line_truncator, wrapped);
}

#[test]
fn line_composer_long_word() {
	let width = 20;
	let text = "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmno";
	let (word_wrapper, _, _) =
		run_composer(Composer::WordWrapper { trim: true }, text, width as u16);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width as u16);

	let wrapped = vec![
		text.get(..width).unwrap(),
		text.get(width..width * 2).unwrap(),
		text.get(width * 2..width * 3).unwrap(),
		text.get(width * 3..).unwrap(),
	];
	assert_eq!(
		word_wrapper, wrapped,
		"WordWrapper should detect the line cannot be broken on word boundary and \
         break it at line width limit."
	);
	assert_eq!(line_truncator, [text.get(..width).unwrap()]);
}

#[test]
fn line_composer_long_sentence() {
	let width = 20;
	let text = "abcd efghij klmnopabcd efgh ijklmnopabcdefg hijkl mnopab c d e f g h i j k l m n o";
	let text_multi_space = "abcd efghij    klmnopabcd efgh     ijklmnopabcdefg hijkl mnopab c d e f g h i j k l \
         m n o";
	let (word_wrapper_single_space, _, _) =
		run_composer(Composer::WordWrapper { trim: true }, text, width as u16);
	let (word_wrapper_multi_space, _, _) = run_composer(
		Composer::WordWrapper { trim: true },
		text_multi_space,
		width as u16,
	);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width as u16);

	let word_wrapped = vec![
		"abcd efghij",
		"klmnopabcd efgh",
		"ijklmnopabcdefg",
		"hijkl mnopab c d e f",
		"g h i j k l m n o",
	];
	assert_eq!(word_wrapper_single_space, word_wrapped);
	assert_eq!(word_wrapper_multi_space, word_wrapped);

	assert_eq!(line_truncator, [text.get(..width).unwrap()]);
}

#[test]
fn line_composer_zero_width() {
	let width = 0;
	let text = "abcd efghij klmnopabcd efgh ijklmnopabcdefg hijkl mnopab ";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);

	let expected: Vec<&str> = Vec::new();
	assert_eq!(word_wrapper, expected);
	assert_eq!(line_truncator, expected);
}

#[test]
fn line_composer_max_line_width_of_1() {
	let width = 1;
	let text = "abcd efghij klmnopabcd efgh ijklmnopabcdefg hijkl mnopab ";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);

	let expected: Vec<&str> = UnicodeSegmentation::graphemes(text, true)
		.filter(|g| g.chars().any(|c| !c.is_whitespace()))
		.collect();
	assert_eq!(word_wrapper, expected);
	assert_eq!(line_truncator, ["a"]);
}

#[test]
fn line_composer_max_line_width_of_1_double_width_characters() {
	let width = 1;
	let text = "コンピュータ上で文字を扱う場合、典型的には文字\naaa\naによる通信を行う場合にその\
                両端点では、";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
	assert_eq!(word_wrapper, ["", "a", "a", "a", "a"]);
	assert_eq!(line_truncator, ["", "a", "a"]);
}

/// Tests `WordWrapper` with words some of which exceed line length and some not.
#[test]
fn line_composer_word_wrapper_mixed_length() {
	let width = 20;
	let text = "abcd efghij klmnopabcdefghijklmnopabcdefghijkl mnopab cdefghi j klmno";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	assert_eq!(
		word_wrapper,
		vec![
			"abcd efghij",
			"klmnopabcdefghijklmn",
			"opabcdefghijkl",
			"mnopab cdefghi j",
			"klmno",
		]
	);
}

#[test]
fn line_composer_double_width_chars() {
	let width = 20;
	let text = "コンピュータ上で文字を扱う場合、典型的には文字による通信を行う場合にその両端点\
                では、";
	let (word_wrapper, word_wrapper_width, _) =
		run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
	assert_eq!(line_truncator, ["コンピュータ上で文字"]);
	let wrapped = [
		"コンピュータ上で文字",
		"を扱う場合、典型的に",
		"は文字による通信を行",
		"う場合にその両端点で",
		"は、",
	];
	assert_eq!(word_wrapper, wrapped);
	assert_eq!(word_wrapper_width, [width, width, width, width, 4]);
}

#[test]
fn line_composer_leading_whitespace_removal() {
	let width = 20;
	let text = "AAAAAAAAAAAAAAAAAAAA    AAA";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
	assert_eq!(word_wrapper, ["AAAAAAAAAAAAAAAAAAAA", "AAA"]);
	assert_eq!(line_truncator, ["AAAAAAAAAAAAAAAAAAAA"]);
}

/// Tests truncation of leading whitespace.
#[test]
fn line_composer_lots_of_spaces() {
	let width = 20;
	let text = "                                                                     ";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
	assert_eq!(word_wrapper, [""]);
	assert_eq!(line_truncator, ["                    "]);
}

/// Tests an input starting with a letter, followed by spaces - some of the behaviour is
/// incidental.
#[test]
fn line_composer_char_plus_lots_of_spaces() {
	let width = 20;
	let text = "a                                                                     ";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
	assert_eq!(word_wrapper, ["a", ""]);
	assert_eq!(line_truncator, ["a                   "]);
}

#[test]
fn line_composer_word_wrapper_double_width_chars_mixed_with_spaces() {
	let width = 20;
	let text =
		"コンピュ ータ上で文字を扱う場合、 典型的には文 字による 通信を行 う場合にその両端点では、";
	let (word_wrapper, word_wrapper_width, _) =
		run_composer(Composer::WordWrapper { trim: true }, text, width);
	assert_eq!(
		word_wrapper,
		vec![
			"コンピュ",
			"ータ上で文字を扱う場",
			"合、 典型的には文",
			"字による 通信を行",
			"う場合にその両端点で",
			"は、",
		]
	);
	// Odd-sized lines have a space in them.
	assert_eq!(word_wrapper_width, [8, 20, 17, 17, 20, 4]);
}

/// Ensure words separated by nbsp are wrapped as if they were a single one.
#[test]
fn line_composer_word_wrapper_nbsp() {
	let width = 20;
	let text = "AAAAAAAAAAAAAAA AAAA\u{00a0}AAA";
	let (word_wrapper, word_wrapper_widths, _) =
		run_composer(Composer::WordWrapper { trim: true }, text, width);
	assert_eq!(word_wrapper, ["AAAAAAAAAAAAAAA", "AAAA\u{00a0}AAA"]);
	assert_eq!(word_wrapper_widths, [15, 8]);

	// Ensure that if the character was a regular space, it would be wrapped differently.
	let text_space = text.replace('\u{00a0}', " ");
	let (word_wrapper_space, word_wrapper_widths, _) =
		run_composer(Composer::WordWrapper { trim: true }, text_space, width);
	assert_eq!(word_wrapper_space, ["AAAAAAAAAAAAAAA AAAA", "AAA"]);
	assert_eq!(word_wrapper_widths, [20, 3]);
}

#[test]
fn line_composer_word_wrapper_preserve_indentation() {
	let width = 20;
	let text = "AAAAAAAAAAAAAAAAAAAA    AAA";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: false }, text, width);
	assert_eq!(word_wrapper, ["AAAAAAAAAAAAAAAAAAAA", "   AAA"]);
}

#[test]
fn line_composer_word_wrapper_preserve_indentation_with_wrap() {
	let width = 10;
	let text = "AAA AAA AAAAA AA AAAAAA\n B\n  C\n   D";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: false }, text, width);
	assert_eq!(
		word_wrapper,
		vec!["AAA AAA", "AAAAA AA", "AAAAAA", " B", "  C", "   D"]
	);
}

#[test]
fn line_composer_word_wrapper_preserve_indentation_lots_of_whitespace() {
	let width = 10;
	let text = "               4 Indent\n                 must wrap!";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: false }, text, width);
	assert_eq!(
		word_wrapper,
		vec![
			"          ",
			"    4",
			"Indent",
			"          ",
			"      must",
			"wrap!"
		]
	);
}

#[test]
fn line_composer_zero_width_at_end() {
	let width = 3;
	let line = "foo\u{200B}";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, line, width);
	let (line_truncator, _, _) = run_composer(Composer::LineTruncator, line, width);
	assert_eq!(word_wrapper, ["foo"]);
	assert_eq!(line_truncator, ["foo\u{200B}"]);
}

#[test]
fn line_composer_preserves_line_alignment() {
	let width = 20;
	let lines = vec![
		Line::from("Something that is left aligned.").alignment(HorizontalAlignment::Left),
		Line::from("This is right aligned and half short.").alignment(HorizontalAlignment::Right),
		Line::from("This should sit in the center.").alignment(HorizontalAlignment::Center),
	];
	let (_, _, wrapped_alignments) =
		run_composer(Composer::WordWrapper { trim: true }, lines.clone(), width);
	let (_, _, truncated_alignments) = run_composer(Composer::LineTruncator, lines, width);
	assert_eq!(
		wrapped_alignments,
		vec![
			HorizontalAlignment::Left,
			HorizontalAlignment::Left,
			HorizontalAlignment::Right,
			HorizontalAlignment::Right,
			HorizontalAlignment::Right,
			HorizontalAlignment::Center,
			HorizontalAlignment::Center
		]
	);
	assert_eq!(
		truncated_alignments,
		vec![
			HorizontalAlignment::Left,
			HorizontalAlignment::Right,
			HorizontalAlignment::Center
		]
	);
}

#[test]
fn line_composer_zero_width_white_space() {
	let width = 3;
	let line = "foo\u{200b}bar";
	let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, line, width);
	assert_eq!(word_wrapper, ["foo", "bar"]);
}
