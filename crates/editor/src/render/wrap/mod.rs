//! Line wrapping with sticky punctuation.
//!
//! Soft wraps text keeping punctuation attached to words:
//! - Trailing punctuation (`. , ; : ! ? ) ] }`) stays with preceding word
//! - Leading punctuation (`( [ { @ # $`) stays with following word
//! - Path separators (`- /`) remain breakable

#[cfg(test)]
mod tests;

/// A segment of a wrapped line.
pub struct WrapSegment {
	/// The text content of this segment.
	pub text: String,
	/// Character offset from the start of the original line.
	pub start_offset: usize,
}

/// Wraps a line of text into segments that fit within a maximum width.
///
/// Breaks at word boundaries when possible, keeping punctuation attached
/// to their associated words (sticky punctuation).
pub fn wrap_line(line: &str, max_width: usize, tab_width: usize) -> Vec<WrapSegment> {
	if max_width == 0 {
		return vec![];
	}

	let chars: Vec<char> = line.chars().collect();
	if chars.is_empty() {
		return vec![];
	}

	let mut segments = Vec::new();
	let mut pos = 0;

	while pos < chars.len() {
		let mut col = 0usize;
		let mut end = pos;

		while end < chars.len() {
			let ch = chars[end];
			let mut w = if ch == '\t' {
				tab_width.saturating_sub(col % tab_width)
			} else {
				1
			};
			if w == 0 {
				w = 1;
			}

			let remaining = max_width.saturating_sub(col);
			if remaining == 0 {
				break;
			}
			if w > remaining {
				w = remaining;
			}

			col += w;
			end += 1;
			if col >= max_width {
				break;
			}
		}

		if end == pos {
			end = (pos + 1).min(chars.len());
		}

		let break_pos = if end < chars.len() {
			let candidate = find_wrap_break(&chars, pos, end);
			if candidate > pos { candidate } else { end }
		} else {
			chars.len()
		};

		segments.push(WrapSegment {
			text: chars[pos..break_pos].iter().collect(),
			start_offset: pos,
		});

		pos = break_pos;
	}

	segments
}

fn is_trailing_punct(ch: char) -> bool {
	matches!(
		ch,
		'.' | ',' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '>' | '"' | '\'' | '`'
	)
}

fn is_leading_punct(ch: char) -> bool {
	matches!(
		ch,
		'(' | '[' | '{' | '<' | '@' | '#' | '$' | '"' | '\'' | '`'
	)
}

fn can_break_after(chars: &[char], i: usize) -> bool {
	let ch = chars[i];

	if ch == ' ' || ch == '\t' {
		return true;
	}

	let Some(&next_ch) = chars.get(i + 1) else {
		return true;
	};

	// Keep "word." together - don't break before trailing punct
	if is_trailing_punct(next_ch) && !next_ch.is_whitespace() {
		return false;
	}

	// Keep "(word" together - don't break after leading punct
	if is_leading_punct(ch) {
		return false;
	}

	// Break after trailing punct when followed by new word unit
	if is_trailing_punct(ch) {
		return next_ch.is_whitespace() || is_leading_punct(next_ch) || next_ch.is_alphanumeric();
	}

	// Path separators remain breakable
	ch == '-' || ch == '/'
}

fn find_wrap_break(chars: &[char], start: usize, max_end: usize) -> usize {
	let search_start = start + (max_end - start) / 2;

	for i in (search_start..max_end).rev() {
		if can_break_after(chars, i) {
			return i + 1;
		}
	}

	max_end
}
