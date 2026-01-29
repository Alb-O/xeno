use xeno_primitives::Rope;
use xeno_primitives::range::CharIdx;

/// Doc-derived line content slice with offset-based access.
///
/// Holds character and byte offsets into the rope rather than owned text,
/// enabling zero-copy line access during rendering.
#[derive(Debug, Clone)]
pub struct LineSlice {
	pub line_idx: usize,
	pub start_char: CharIdx,
	pub end_char_incl_nl: CharIdx,
	pub content_end_char: CharIdx,
	pub start_byte: u32,
	pub has_newline: bool,
}

impl LineSlice {
	/// Returns a [`RopeSlice`] for the content portion of the line (excluding newline).
	pub fn content_slice<'a>(&self, rope: &'a Rope) -> xeno_primitives::RopeSlice<'a> {
		rope.slice(self.start_char..self.content_end_char)
	}

	/// Returns the line content as a String.
	///
	/// Prefer [`content_slice`] for zero-copy access where possible.
	pub fn content_string(&self, rope: &Rope) -> String {
		self.content_slice(rope).into()
	}
}

pub struct LineSource;

impl LineSource {
	/// Loads a line slice from a rope at the specified index.
	///
	/// Returns `None` if the line index is out of bounds.
	pub fn load(rope: &Rope, line_idx: usize) -> Option<LineSlice> {
		if line_idx >= rope.len_lines() {
			return None;
		}

		let start_char = rope.line_to_char(line_idx);
		let start_byte = rope.char_to_byte(start_char) as u32;
		let line_len = rope.line(line_idx).len_chars();
		let end_char_incl_nl = start_char + line_len;

		let line_slice = rope.line(line_idx);
		let has_newline = if line_len > 0 {
			let last_char = line_slice.char(line_len.saturating_sub(1));
			last_char == '\n'
		} else {
			false
		};

		let content_end_char = if has_newline {
			end_char_incl_nl.saturating_sub(1)
		} else {
			end_char_incl_nl
		};

		Some(LineSlice {
			line_idx,
			start_char,
			end_char_incl_nl,
			content_end_char,
			start_byte,
			has_newline,
		})
	}
}
