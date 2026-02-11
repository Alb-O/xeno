//! Line wrapping with sticky punctuation.
//!
//! Soft wraps text keeping punctuation attached to words:
//! - Trailing punctuation (`. , ; : ! ? ) ] }`) stays with preceding word
//! - Leading punctuation (`( [ { @ # $`) stays with following word
//! - Path separators (`- /`) remain breakable

use unicode_width::UnicodeWidthChar;
use xeno_primitives::RopeSlice;

#[cfg(test)]
mod tests;

/// Calculates the display width of a character at a given column position.
///
/// This is the single source of truth for character widths across the rendering
/// pipeline. It handles:
/// - Tabs: variable width to reach next tab stop
/// - Unicode: uses UnicodeWidthChar for CJK, emoji, etc.
pub fn cell_width(ch: char, col: usize, tab_width: usize) -> usize {
	match ch {
		'\t' => tab_width.saturating_sub(col % tab_width).max(1),
		_ => UnicodeWidthChar::width(ch).unwrap_or(1).max(1),
	}
}

/// A segment of a wrapped line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrappedSegment {
	/// Character offset from the start of the original line.
	pub start_char_offset: usize,
	/// Length of this segment in characters.
	pub char_len: usize,
	/// Visual indent width to prepend for this segment (0 for first segment).
	pub indent_cols: usize,
}

/// A segment of a wrapped line with owned text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapSegment {
	/// The text content of this segment.
	pub text: String,
	/// Character offset from the start of the original line.
	pub start_offset: usize,
	/// Visual indent width to prepend for this segment (0 for first segment).
	pub indent_cols: usize,
}

/// Wraps a line of text into ranges that fit within a maximum width.
///
/// # Empty Lines
/// Returns a single segment of length 0 for empty strings.
pub fn wrap_line_ranges(line: &str, max_width: usize, tab_width: usize) -> Vec<WrappedSegment> {
	if max_width == 0 {
		return vec![];
	}

	let chars: Vec<char> = line.chars().collect();
	if chars.is_empty() {
		return vec![WrappedSegment {
			start_char_offset: 0,
			char_len: 0,
			indent_cols: 0,
		}];
	}

	const MIN_CONTINUATION_CONTENT: usize = 20;

	let raw_indent = leading_indent_width(&chars, tab_width);
	let has_room = max_width.saturating_sub(raw_indent) >= MIN_CONTINUATION_CONTENT;
	let indent_cols = if has_room { raw_indent } else { 0 };
	let continuation_width = max_width - indent_cols;

	let mut segments = Vec::new();
	let mut pos = 0;
	let mut is_first = true;

	while pos < chars.len() {
		let effective_width = if is_first { max_width } else { continuation_width };
		let mut col = 0usize;
		let mut end = pos;

		while end < chars.len() {
			let ch = chars[end];
			let w = cell_width(ch, col, tab_width);

			let remaining = effective_width.saturating_sub(col);
			if remaining == 0 {
				break;
			}
			if w > remaining {
				break;
			}

			col += w;
			end += 1;
			if col >= effective_width {
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

		segments.push(WrappedSegment {
			start_char_offset: pos,
			char_len: break_pos - pos,
			indent_cols: if is_first { 0 } else { indent_cols },
		});

		pos = break_pos;
		is_first = false;
	}

	segments
}

/// Wraps a line of text into segments that fit within a maximum width.
///
/// Breaks at word boundaries when possible, keeping punctuation attached
/// to their associated words (sticky punctuation).
///
/// Continuation lines (after the first segment) are indented to match the
/// leading whitespace of the original line, creating visually aligned wrapped text.
pub fn wrap_line(line: &str, max_width: usize, tab_width: usize) -> Vec<WrapSegment> {
	let chars: Vec<char> = line.chars().collect();
	wrap_line_ranges(line, max_width, tab_width)
		.into_iter()
		.map(|s| WrapSegment {
			text: chars[s.start_char_offset..s.start_char_offset + s.char_len].iter().collect(),
			start_offset: s.start_char_offset,
			indent_cols: s.indent_cols,
		})
		.collect()
}

/// Calculates the visual width of leading whitespace (spaces and tabs).
fn leading_indent_width(chars: &[char], tab_width: usize) -> usize {
	let mut col = 0;
	for &ch in chars {
		if ch == ' ' || ch == '\t' {
			col += cell_width(ch, col, tab_width);
		} else {
			break;
		}
	}
	col
}

fn is_trailing_punct(ch: char) -> bool {
	matches!(ch, '.' | ',' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '>' | '"' | '\'' | '`')
}

fn is_leading_punct(ch: char) -> bool {
	matches!(ch, '(' | '[' | '{' | '<' | '@' | '#' | '$' | '"' | '\'' | '`')
}

fn can_break_after(chars: &[char], i: usize) -> bool {
	let ch = chars[i];

	if ch == ' ' || ch == '\t' {
		return true;
	}

	let Some(&next_ch) = chars.get(i + 1) else {
		return true;
	};

	if is_trailing_punct(next_ch) && !next_ch.is_whitespace() {
		return false;
	}
	if is_leading_punct(ch) {
		return false;
	}
	if is_trailing_punct(ch) {
		return next_ch.is_whitespace() || is_leading_punct(next_ch) || next_ch.is_alphanumeric();
	}

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

/// Wraps a line from a [`RopeSlice`] into ranges that fit within a maximum width.
///
/// Operates directly on the rope content to avoid string allocations. Uses a
/// forward scan with best-break tracking to find optimal wrap points.
///
/// # Empty Lines
/// Returns a single segment of length 0 for empty lines, ensuring they can be
/// rendered with a cursor.
pub fn wrap_line_ranges_rope(line: RopeSlice<'_>, max_width: usize, tab_width: usize) -> Vec<WrappedSegment> {
	if max_width == 0 {
		return vec![];
	}

	let line_len = line.len_chars();
	if line_len == 0 {
		return vec![WrappedSegment {
			start_char_offset: 0,
			char_len: 0,
			indent_cols: 0,
		}];
	}

	const MIN_CONTINUATION_CONTENT: usize = 20;

	let raw_indent = leading_indent_width_rope(&line, tab_width);
	let has_room = max_width.saturating_sub(raw_indent) >= MIN_CONTINUATION_CONTENT;
	let indent_cols = if has_room { raw_indent } else { 0 };
	let continuation_width = max_width - indent_cols;

	let mut segments = Vec::new();
	let mut pos = 0;
	let mut is_first = true;

	while pos < line_len {
		let effective_width = if is_first { max_width } else { continuation_width };

		let mut col = 0usize;
		let mut end = pos;
		let mut best_break = pos;

		let mut iter = line.slice(pos..).chars().peekable();
		while let Some(ch) = iter.next() {
			let w = cell_width(ch, col, tab_width);

			let remaining = effective_width.saturating_sub(col);
			if remaining == 0 || w > remaining {
				break;
			}

			col += w;
			end += 1;

			let next_ch = iter.peek().copied();
			if can_break_after_rope(ch, next_ch) {
				best_break = end;
			}

			if col >= effective_width {
				break;
			}
		}

		if end == pos {
			end = (pos + 1).min(line_len);
			best_break = end;
		} else if best_break == pos {
			best_break = end;
		}

		segments.push(WrappedSegment {
			start_char_offset: pos,
			char_len: best_break - pos,
			indent_cols: if is_first { 0 } else { indent_cols },
		});

		pos = best_break;
		is_first = false;
	}

	segments
}

/// Calculates the visual width of leading whitespace from a RopeSlice.
fn leading_indent_width_rope(line: &RopeSlice<'_>, tab_width: usize) -> usize {
	let mut col = 0;
	for ch in line.chars() {
		if ch == ' ' || ch == '\t' {
			col += cell_width(ch, col, tab_width);
		} else {
			break;
		}
	}
	col
}

/// Checks if we can break after a character, given the next character.
fn can_break_after_rope(ch: char, next_ch: Option<char>) -> bool {
	if ch == ' ' || ch == '\t' {
		return true;
	}

	let Some(next) = next_ch else {
		return true;
	};

	if is_trailing_punct(next) && !next.is_whitespace() {
		return false;
	}
	if is_leading_punct(ch) {
		return false;
	}
	if is_trailing_punct(ch) {
		return next.is_whitespace() || is_leading_punct(next) || next.is_alphanumeric();
	}

	ch == '-' || ch == '/'
}
