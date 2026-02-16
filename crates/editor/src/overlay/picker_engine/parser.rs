//! Char-index-safe token parsing helpers for picker inputs.

/// Lossless token span for picker command lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerToken {
	pub start: usize,
	pub end: usize,
	pub content_start: usize,
	pub content_end: usize,
	pub quoted: Option<char>,
	pub close_quote_idx: Option<usize>,
}

/// Tokenizes whitespace-delimited picker input with quote support.
pub fn tokenize(chars: &[char]) -> Vec<PickerToken> {
	let mut out = Vec::new();
	let mut i = 0usize;
	while i < chars.len() {
		while i < chars.len() && chars[i].is_whitespace() {
			i += 1;
		}
		if i >= chars.len() {
			break;
		}

		let start = i;
		if chars[i] == '"' || chars[i] == '\'' {
			let quote = chars[i];
			i += 1;
			let content_start = i;
			while i < chars.len() && chars[i] != quote {
				i += 1;
			}
			let content_end = i.min(chars.len());
			let close_quote_idx = if i < chars.len() && chars[i] == quote { Some(i) } else { None };
			if close_quote_idx.is_some() {
				i += 1;
			}
			out.push(PickerToken {
				start,
				end: i,
				content_start,
				content_end,
				quoted: Some(quote),
				close_quote_idx,
			});
		} else {
			let content_start = i;
			while i < chars.len() && !chars[i].is_whitespace() {
				i += 1;
			}
			out.push(PickerToken {
				start,
				end: i,
				content_start,
				content_end: i,
				quoted: None,
				close_quote_idx: None,
			});
		}
	}
	out
}

/// Computes replacement end, preserving closing quote when cursor moves past it.
pub fn effective_replace_end(token: &PickerToken, cursor: usize) -> usize {
	match (token.quoted, token.close_quote_idx) {
		(Some(_), Some(close_quote_idx)) if cursor > close_quote_idx => close_quote_idx,
		_ => cursor,
	}
}
