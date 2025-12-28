//! Regex-based search in document.

use evildoer_base::range::CharIdx;
use regex::Regex;
pub use regex::escape as escape_pattern;
use ropey::RopeSlice;

use crate::Range;

/// Check if text matches a regex pattern.
pub fn matches_pattern(text: &str, pattern: &str) -> Result<bool, regex::Error> {
	let re = Regex::new(pattern)?;
	Ok(re.is_match(text))
}

/// Find all matches of a pattern in text.
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

/// Find the next match after the given position.
pub fn find_next(
	text: RopeSlice,
	pattern: &str,
	pos: CharIdx,
) -> Result<Option<Range>, regex::Error> {
	let re = Regex::new(pattern)?;
	let text_str: String = text.chars().collect();

	let byte_pos = char_to_byte_offset(&text_str, pos);
	if byte_pos < text_str.len()
		&& let Some(m) = re.find(&text_str[byte_pos..])
	{
		let start = pos + byte_to_char_offset(&text_str[byte_pos..], m.start());
		let end = pos + byte_to_char_offset(&text_str[byte_pos..], m.end());
		return Ok(Some(Range::new(start, end)));
	}

	// Wrap around: search from start to pos
	if let Some(m) = re.find(&text_str) {
		let start = byte_to_char_offset(&text_str, m.start());
		let end = byte_to_char_offset(&text_str, m.end());
		if start < pos {
			return Ok(Some(Range::new(start, end)));
		}
	}

	Ok(None)
}

/// Find the previous match before the given position.
pub fn find_prev(
	text: RopeSlice,
	pattern: &str,
	pos: CharIdx,
) -> Result<Option<Range>, regex::Error> {
	let re = Regex::new(pattern)?;
	let text_str: String = text.chars().collect();

	// Find all matches before pos
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
		return Ok(last_before);
	}

	// Wrap around: find last match in document
	let mut last: Option<Range> = None;
	for m in re.find_iter(&text_str) {
		let start = byte_to_char_offset(&text_str, m.start());
		let end = byte_to_char_offset(&text_str, m.end());
		last = Some(Range::new(start, end));
	}

	Ok(last)
}

fn byte_to_char_offset(s: &str, byte_offset: usize) -> CharIdx {
	s[..byte_offset].chars().count()
}

fn char_to_byte_offset(s: &str, char_offset: CharIdx) -> usize {
	s.char_indices()
		.nth(char_offset)
		.map(|(i, _)| i)
		.unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_find_next() {
		let text = Rope::from("hello world hello");
		let slice = text.slice(..);

		let m = find_next(slice, "hello", 0).unwrap().unwrap();
		assert_eq!(m.min(), 0);
		assert_eq!(m.max(), 5);

		let m = find_next(slice, "hello", 1).unwrap().unwrap();
		assert_eq!(m.min(), 12);
		assert_eq!(m.max(), 17);

		let m = find_next(slice, "hello", 13).unwrap().unwrap();
		assert_eq!(m.min(), 0);
	}

	#[test]
	fn test_find_prev() {
		let text = Rope::from("hello world hello");
		let slice = text.slice(..);

		let m = find_prev(slice, "hello", 17).unwrap().unwrap();
		assert_eq!(m.min(), 12);

		let m = find_prev(slice, "hello", 12).unwrap().unwrap();
		assert_eq!(m.min(), 0);

		let m = find_prev(slice, "hello", 0).unwrap().unwrap();
		assert_eq!(m.min(), 12);
	}

	#[test]
	fn test_find_all_matches() {
		let text = Rope::from("foo bar foo baz foo");
		let slice = text.slice(..);

		let matches = find_all_matches(slice, "foo").unwrap();
		assert_eq!(matches.len(), 3);
		assert_eq!(matches[0].min(), 0);
		assert_eq!(matches[1].min(), 8);
		assert_eq!(matches[2].min(), 16);
	}
}
