use evildoer_tui::widgets::Paragraph;

/// A segment of a wrapped line.
pub struct WrapSegment {
	/// The text content of this segment.
	pub text: String,
	/// Character offset from the start of the original line.
	pub start_offset: usize,
}

/// Result of rendering a buffer's content.
pub struct RenderResult {
	/// The rendered paragraph widget ready for display.
	pub widget: Paragraph<'static>,
}

/// Wraps a line of text into multiple segments based on maximum width.
///
/// This is a standalone version of the wrapping logic that can be used
/// by both `Buffer` and `Editor`.
pub fn wrap_line(line: &str, max_width: usize) -> Vec<WrapSegment> {
	if max_width == 0 {
		return vec![];
	}

	let chars: Vec<char> = line.chars().collect();
	if chars.is_empty() {
		return vec![];
	}

	let tab_width = 4usize;

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
			if candidate > pos {
				candidate
			} else {
				end
			}
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
fn find_wrap_break(chars: &[char], start: usize, max_end: usize) -> usize {
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
