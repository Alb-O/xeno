use super::super::types::WrapSegment;
use crate::Editor;

impl Editor {
	/// Wraps a line of text into multiple segments based on maximum width.
	///
	/// This function breaks long lines into multiple visual segments that fit within
	/// the specified width. It attempts to break at word boundaries when possible.
	///
	/// # Parameters
	/// - `line`: The text to wrap
	/// - `max_width`: Maximum width in characters for each segment
	/// - `tab_width`: Number of spaces a tab character occupies
	///
	/// # Returns
	/// A vector of [`WrapSegment`]s, each containing a portion of the line and its
	/// character offset from the start of the line. Returns an empty vector if
	/// `max_width` is 0 or the line is empty.
	pub fn wrap_line(&self, line: &str, max_width: usize, tab_width: usize) -> Vec<WrapSegment> {
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
				let candidate = self.find_wrap_break(&chars, pos, end);
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

	/// Finds an optimal break point for wrapping text.
	///
	/// Searches backward from the maximum end position to find a natural break point
	/// such as whitespace or after punctuation marks. This provides better visual
	/// wrapping compared to hard breaks at the width limit.
	///
	/// # Parameters
	/// - `chars`: The character array to search
	/// - `start`: Starting position of the current segment
	/// - `max_end`: Maximum position where the break can occur
	///
	/// # Returns
	/// The position where the line should break. If no natural break point is found,
	/// returns `max_end`.
	pub(crate) fn find_wrap_break(&self, chars: &[char], start: usize, max_end: usize) -> usize {
		let search_start = start + (max_end - start) / 2;

		for i in (search_start..max_end).rev() {
			let ch = chars[i];
			if ch == ' ' || ch == '\t' {
				return i + 1;
			}
			if i + 1 < chars.len() {
				let next = chars[i + 1];
				if next == '-' || next == '/' || next == '.' || next == ',' {
					return i + 1;
				}
			}
		}

		max_end
	}
}
