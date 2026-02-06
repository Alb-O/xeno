use super::*;

#[test]
fn test_utf32_round_trip() {
	let text = Rope::from("hello\nworld\n");
	let encoding = OffsetEncoding::Utf32;

	// First line
	let pos = Position {
		line: 0,
		character: 3,
	};
	let char_idx = lsp_position_to_char(&text, pos, encoding).unwrap();
	assert_eq!(char_idx, 3);
	assert_eq!(
		char_to_lsp_position(&text, char_idx, encoding).unwrap(),
		pos
	);

	// Second line
	let pos = Position {
		line: 1,
		character: 2,
	};
	let char_idx = lsp_position_to_char(&text, pos, encoding).unwrap();
	assert_eq!(char_idx, 8); // "hello\n" = 6 chars, + 2 = 8
	assert_eq!(
		char_to_lsp_position(&text, char_idx, encoding).unwrap(),
		pos
	);
}

#[test]
fn test_utf16_with_emoji() {
	// Emoji like U+1F600 is a single Unicode codepoint but 2 UTF-16 code units
	let text = Rope::from("a\u{1F600}b\n");
	let encoding = OffsetEncoding::Utf16;

	// Position after 'a' (before emoji)
	let pos = Position {
		line: 0,
		character: 1,
	};
	let char_idx = lsp_position_to_char(&text, pos, encoding).unwrap();
	assert_eq!(char_idx, 1);

	// Position after emoji (in UTF-16, this is character 3 because emoji takes 2 units)
	let pos = Position {
		line: 0,
		character: 3,
	};
	let char_idx = lsp_position_to_char(&text, pos, encoding).unwrap();
	assert_eq!(char_idx, 2); // In rope chars: a=0, U+1F600=1, b=2

	// Convert back: char index 2 should give UTF-16 column 3
	let back = char_to_lsp_position(&text, 2, encoding).unwrap();
	assert_eq!(back.character, 3);
}

#[test]
fn test_utf8_with_multibyte() {
	// e with acute accent is 2 bytes in UTF-8
	let text = Rope::from("caf\u{00E9}\n");
	let encoding = OffsetEncoding::Utf8;

	// Position at 'f' (index 2 in chars, but byte offset 2)
	let pos = Position {
		line: 0,
		character: 3,
	};
	let char_idx = lsp_position_to_char(&text, pos, encoding).unwrap();
	assert_eq!(char_idx, 3); // c=1byte, a=1byte, f=1byte -> 3 bytes = 3 chars here

	// Position at U+00E9 (char index 3, but U+00E9 is at byte offset 4 and takes 2 bytes)
	// Actually "caf\u{00E9}" = c(1) + a(1) + f(1) + \u{00E9}(2) = 5 bytes
	let char_idx = 3; // U+00E9 position
	let back = char_to_lsp_position(&text, char_idx, encoding).unwrap();
	assert_eq!(back.character, 3); // byte offset of U+00E9 is 3
}

#[test]
fn test_out_of_bounds() {
	let text = Rope::from("hello\n");
	let encoding = OffsetEncoding::Utf32;

	// Line out of bounds
	let pos = Position {
		line: 5,
		character: 0,
	};
	assert!(lsp_position_to_char(&text, pos, encoding).is_none());

	// Char index out of bounds
	assert!(char_to_lsp_position(&text, 100, encoding).is_none());
}

#[test]
fn test_clamp_column() {
	let text = Rope::from("hi\n");
	let encoding = OffsetEncoding::Utf32;

	// Column past end of line should clamp
	let pos = Position {
		line: 0,
		character: 100,
	};
	let char_idx = lsp_position_to_char(&text, pos, encoding).unwrap();
	assert_eq!(char_idx, 2); // "hi" has 2 chars, clamped to end
}

#[test]
fn test_range_conversion() {
	let text = Rope::from("hello\nworld\n");
	let encoding = OffsetEncoding::Utf32;

	let lsp_range = Range {
		start: Position {
			line: 0,
			character: 1,
		},
		end: Position {
			line: 1,
			character: 3,
		},
	};

	let (start, end) = lsp_range_to_char_range(&text, lsp_range, encoding).unwrap();
	assert_eq!(start, 1); // 'e' in "hello"
	assert_eq!(end, 9); // 'l' in "world" (6 + 3)

	let back = char_range_to_lsp_range(&text, start, end, encoding).unwrap();
	assert_eq!(back, lsp_range);
}
