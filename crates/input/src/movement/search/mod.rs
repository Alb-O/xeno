//! Regex-based search in document.

use regex::Regex;
pub use regex::escape as escape_pattern;
use ropey::RopeSlice;
use xeno_primitives::{CharIdx, Range};

/// Returns whether `text` matches `pattern` as a regex.
pub fn matches_pattern(text: &str, pattern: &str) -> Result<bool, regex::Error> {
	let re = Regex::new(pattern)?;
	Ok(re.is_match(text))
}

/// Finds all regex matches of `pattern` in `text`.
pub fn find_all_matches(text: RopeSlice, pattern: &str) -> Result<Vec<Range>, regex::Error> {
	let re = Regex::new(pattern)?;
	let text_str: String = text.chars().collect();

	let mut matches = Vec::new();
	for m in re.find_iter(&text_str) {
		let start = byte_to_char_offset(&text_str, m.start());
		let end = byte_to_char_offset(&text_str, m.end());
		matches.push(Range::new(start, end));
	}

	Ok(matches)
}

/// Finds the next regex match of `pattern` after `pos`, with document wraparound.
pub fn find_next(text: RopeSlice, pattern: &str, pos: CharIdx) -> Result<Option<Range>, regex::Error> {
	let re = Regex::new(pattern)?;
	Ok(find_next_re(text, &re, pos))
}

/// Finds the previous regex match of `pattern` before `pos`, with document wraparound.
pub fn find_prev(text: RopeSlice, pattern: &str, pos: CharIdx) -> Result<Option<Range>, regex::Error> {
	let re = Regex::new(pattern)?;
	Ok(find_prev_re(text, &re, pos))
}

/// Finds the next match after `pos` using a precompiled regex, wrapping to
/// the start of the document if no match is found after `pos`.
pub fn find_next_re(text: RopeSlice, re: &Regex, pos: CharIdx) -> Option<Range> {
	let text_str: String = text.chars().collect();
	let byte_pos = char_to_byte_offset(&text_str, pos);

	if byte_pos < text_str.len()
		&& let Some(m) = re.find(&text_str[byte_pos..])
	{
		let start = pos + byte_to_char_offset(&text_str[byte_pos..], m.start());
		let end = pos + byte_to_char_offset(&text_str[byte_pos..], m.end());
		return Some(Range::new(start, end));
	}

	if let Some(m) = re.find(&text_str) {
		let start = byte_to_char_offset(&text_str, m.start());
		let end = byte_to_char_offset(&text_str, m.end());
		if start < pos {
			return Some(Range::new(start, end));
		}
	}

	None
}

/// Finds the previous match before `pos` using a precompiled regex, wrapping
/// to the end of the document if no match is found before `pos`.
pub fn find_prev_re(text: RopeSlice, re: &Regex, pos: CharIdx) -> Option<Range> {
	let text_str: String = text.chars().collect();

	let mut last_before: Option<Range> = None;
	for m in re.find_iter(&text_str) {
		let start = byte_to_char_offset(&text_str, m.start());
		if start < pos {
			let end = byte_to_char_offset(&text_str, m.end());
			last_before = Some(Range::new(start, end));
		} else {
			break;
		}
	}

	if last_before.is_some() {
		return last_before;
	}

	let mut last: Option<Range> = None;
	for m in re.find_iter(&text_str) {
		let start = byte_to_char_offset(&text_str, m.start());
		let end = byte_to_char_offset(&text_str, m.end());
		last = Some(Range::new(start, end));
	}

	last
}

/// Converts a byte offset to a character offset.
fn byte_to_char_offset(s: &str, byte_offset: usize) -> CharIdx {
	s[..byte_offset].chars().count()
}

/// Converts a character offset to a byte offset.
fn char_to_byte_offset(s: &str, char_offset: CharIdx) -> usize {
	s.char_indices().nth(char_offset).map(|(i, _)| i).unwrap_or(s.len())
}

#[cfg(test)]
mod tests;
