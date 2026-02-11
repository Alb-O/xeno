use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Returns terminal-cell width of a string.
pub fn cell_width(s: &str) -> usize {
	s.width()
}

/// Returns terminal-cell width of a single character.
pub fn char_width(c: char) -> usize {
	UnicodeWidthChar::width(c).unwrap_or(0)
}

#[cfg(test)]
mod tests {
	use super::{cell_width, char_width};

	#[test]
	fn width_ascii_is_single_cell() {
		assert_eq!(cell_width("a"), 1);
		assert_eq!(char_width('a'), 1);
	}

	#[test]
	fn width_wide_char_is_two_cells() {
		assert_eq!(cell_width("界"), 2);
		assert_eq!(char_width('界'), 2);
	}
}
