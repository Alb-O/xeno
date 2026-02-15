use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Returns terminal-cell width of a string.
pub fn cell_width(s: &str) -> usize {
	s.width()
}

/// Returns terminal-cell width of a single character.
pub fn char_width(c: char) -> usize {
	UnicodeWidthChar::width(c).unwrap_or(0)
}
