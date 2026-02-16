//! Input mutation helpers for picker completion application.

/// Char-index-safe replacement for a range within an input string.
pub fn replace_char_range(input: &str, start: usize, end: usize, replacement: &str) -> (String, usize) {
	let chars: Vec<char> = input.chars().collect();
	let start = start.min(chars.len());
	let end = end.min(chars.len()).max(start);

	let mut out = String::new();
	for ch in &chars[..start] {
		out.push(*ch);
	}
	out.push_str(replacement);
	for ch in &chars[end..] {
		out.push(*ch);
	}

	let cursor = start + replacement.chars().count();
	(out, cursor)
}
