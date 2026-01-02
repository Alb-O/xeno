//! Tests for Buffer content setting (set_string, set_line, set_style).

use std::dbg;

use itertools::Itertools;
use rstest::{fixture, rstest};

use super::*;

#[test]
fn set_string() {
	let area = Rect::new(0, 0, 5, 1);
	let mut buffer = Buffer::empty(area);

	// Zero-width
	buffer.set_stringn(0, 0, "aaa", 0, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["     "]));

	buffer.set_string(0, 0, "aaa", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["aaa  "]));

	// Width limit:
	buffer.set_stringn(0, 0, "bbbbbbbbbbbbbb", 4, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["bbbb "]));

	buffer.set_string(0, 0, "12345", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["12345"]));

	// Width truncation:
	buffer.set_string(0, 0, "123456", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["12345"]));

	// multi-line
	buffer = Buffer::empty(Rect::new(0, 0, 5, 2));
	buffer.set_string(0, 0, "12345", Style::default());
	buffer.set_string(0, 1, "67890", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["12345", "67890"]));
}

#[test]
fn set_string_multi_width_overwrite() {
	let area = Rect::new(0, 0, 5, 1);
	let mut buffer = Buffer::empty(area);

	// multi-width overwrite
	buffer.set_string(0, 0, "aaaaa", Style::default());
	buffer.set_string(0, 0, "称号", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["称号a"]));
}

#[test]
fn set_string_zero_width() {
	assert_eq!("\u{200B}".width(), 0);

	let area = Rect::new(0, 0, 1, 1);
	let mut buffer = Buffer::empty(area);

	// Leading grapheme with zero width
	let s = "\u{200B}a";
	buffer.set_stringn(0, 0, s, 1, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["a"]));

	// Trailing grapheme with zero with
	let s = "a\u{200B}";
	buffer.set_stringn(0, 0, s, 1, Style::default());
	assert_eq!(buffer, Buffer::with_lines(["a"]));
}

#[test]
fn set_string_double_width() {
	let area = Rect::new(0, 0, 5, 1);
	let mut buffer = Buffer::empty(area);
	buffer.set_string(0, 0, "コン", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["コン "]));

	// Only 1 space left.
	buffer.set_string(0, 0, "コンピ", Style::default());
	assert_eq!(buffer, Buffer::with_lines(["コン "]));
}

#[fixture]
fn small_one_line_buffer() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 5, 1))
}

#[rstest]
#[case::empty("", "     ")]
#[case::one("1", "1    ")]
#[case::full("12345", "12345")]
#[case::overflow("123456", "12345")]
fn set_line_raw(mut small_one_line_buffer: Buffer, #[case] content: &str, #[case] expected: &str) {
	let line = Line::raw(content);
	small_one_line_buffer.set_line(0, 0, &line, 5);

	// note: testing with empty / set_string here instead of with_lines because with_lines calls
	// set_line
	let mut expected_buffer = Buffer::empty(small_one_line_buffer.area);
	expected_buffer.set_string(0, 0, expected, Style::default());
	assert_eq!(small_one_line_buffer, expected_buffer);
}

#[rstest]
#[case::empty("", "     ")]
#[case::one("1", "1    ")]
#[case::full("12345", "12345")]
#[case::overflow("123456", "12345")]
fn set_line_styled(
	mut small_one_line_buffer: Buffer,
	#[case] content: &str,
	#[case] expected: &str,
) {
	let color = Color::Blue;
	let line = Line::styled(content, color);
	small_one_line_buffer.set_line(0, 0, &line, 5);

	// note: manually testing the contents here as the Buffer::with_lines calls set_line
	let actual_contents = small_one_line_buffer
		.content
		.iter()
		.map(Cell::symbol)
		.join("");
	let actual_styles = small_one_line_buffer
		.content
		.iter()
		.map(|c| c.fg)
		.collect_vec();

	// set_line only sets the style for non-empty cells (unlike Line::render which sets the
	// style for all cells)
	let expected_styles = iter::repeat_n(color, content.len().min(5))
		.chain(iter::repeat_n(
			Color::default(),
			5_usize.saturating_sub(content.len()),
		))
		.collect_vec();
	assert_eq!(actual_contents, expected);
	assert_eq!(actual_styles, expected_styles);
}

#[test]
fn set_style() {
	let mut buffer = Buffer::with_lines(["aaaaa", "bbbbb", "ccccc"]);
	buffer.set_style(Rect::new(0, 1, 5, 1), Style::new().red());
	#[rustfmt::skip]
        let expected = Buffer::with_lines([
            "aaaaa".into(),
            "bbbbb".red(),
            "ccccc".into(),
        ]);
	assert_eq!(buffer, expected);
}

#[test]
fn set_style_does_not_panic_when_out_of_area() {
	let mut buffer = Buffer::with_lines(["aaaaa", "bbbbb", "ccccc"]);
	buffer.set_style(Rect::new(0, 1, 10, 3), Style::new().red());
	#[rustfmt::skip]
        let expected = Buffer::with_lines([
            "aaaaa".into(),
            "bbbbb".red(),
            "ccccc".red(),
        ]);
	assert_eq!(buffer, expected);
}

#[test]
fn control_sequence_rendered_full() {
	let text = "I \x1b[0;36mwas\x1b[0m here!";

	let mut buffer = Buffer::filled(Rect::new(0, 0, 25, 3), Cell::new("x"));
	buffer.set_string(1, 1, text, Style::new());

	let expected = Buffer::with_lines([
		"xxxxxxxxxxxxxxxxxxxxxxxxx",
		"xI [0;36mwas[0m here!xxxx",
		"xxxxxxxxxxxxxxxxxxxxxxxxx",
	]);
	assert_eq!(buffer, expected);
}

#[test]
fn control_sequence_rendered_partially() {
	let text = "I \x1b[0;36mwas\x1b[0m here!";

	let mut buffer = Buffer::filled(Rect::new(0, 0, 11, 3), Cell::new("x"));
	buffer.set_string(1, 1, text, Style::new());

	#[rustfmt::skip]
        let expected = Buffer::with_lines([
            "xxxxxxxxxxx",
            "xI [0;36mwa",
            "xxxxxxxxxxx",
        ]);
	assert_eq!(buffer, expected);
}

/// Emojis normally contain various characters which should stay part of the Emoji.
/// This should work fine by utilizing `unicode_segmentation` but a testcase is probably helpful
/// due to the nature of never perfect Unicode implementations and all of its quirks.
#[rstest]
// Shrug without gender or skintone. Has a width of 2 like all emojis have.
#[case::shrug("🤷", "🤷xxxxx")]
// Technically this is a (brown) bear, a zero-width joiner and a snowflake
// As it is joined its a single emoji and should therefore have a width of 2.
// Prior to unicode-width 0.2, this was incorrectly detected as width 4 for some reason
#[case::polarbear("🐻‍❄️", "🐻‍❄️xxxxx")]
// Technically this is an eye, a zero-width joiner and a speech bubble
// Both eye and speech bubble include a 'display as emoji' variation selector
// Prior to unicode-width 0.2, this was incorrectly detected as width 4 for some reason
#[case::eye_speechbubble("👁️‍🗨️", "👁️‍🗨️xxxxx")]
// Keyboard keycap emoji: base symbol + VS16 for emoji presentation
// This should render as a single grapheme with width 2.
#[case::keyboard_emoji("⌨️", "⌨️xxxxx")]
fn renders_emoji(#[case] input: &str, #[case] expected: &str) {
	use unicode_width::UnicodeWidthChar;

	dbg!(input);
	dbg!(input.len());
	dbg!(input
		.graphemes(true)
		.map(|symbol| (symbol, symbol.escape_unicode().to_string(), symbol.width()))
		.collect::<Vec<_>>());
	dbg!(input
		.chars()
		.map(|char| (
			char,
			char.escape_unicode().to_string(),
			char.width(),
			char.is_control()
		))
		.collect::<Vec<_>>());

	let mut buffer = Buffer::filled(Rect::new(0, 0, 7, 1), Cell::new("x"));
	buffer.set_string(0, 0, input, Style::new());

	let expected = Buffer::with_lines([expected]);
	assert_eq!(buffer, expected);
}
