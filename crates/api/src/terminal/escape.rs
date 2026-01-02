//! Terminal escape sequence parsing.

use evildoer_registry::panels::SplitCursorStyle;

/// Parses DECSCUSR (Set Cursor Style): `ESC [ Ps SP q`
pub fn parse_decscusr(bytes: &[u8]) -> Option<SplitCursorStyle> {
	let mut i = 0;
	while i + 4 <= bytes.len() {
		if bytes[i] == 0x1b && bytes[i + 1] == b'[' {
			let start = i + 2;
			let mut end = start;
			while end < bytes.len() && bytes[end].is_ascii_digit() {
				end += 1;
			}
			if end + 2 <= bytes.len() && bytes[end] == b' ' && bytes[end + 1] == b'q' {
				let ps = std::str::from_utf8(&bytes[start..end])
					.ok()
					.and_then(|s| s.parse::<u8>().ok())
					.unwrap_or(0);
				return Some(match ps {
					0 | 1 => SplitCursorStyle::BlinkingBlock,
					2 => SplitCursorStyle::Block,
					3 => SplitCursorStyle::BlinkingUnderline,
					4 => SplitCursorStyle::Underline,
					5 => SplitCursorStyle::BlinkingBar,
					6 => SplitCursorStyle::Bar,
					_ => SplitCursorStyle::Default,
				});
			}
		}
		i += 1;
	}
	None
}
