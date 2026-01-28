use xeno_primitives::Rope;
use xeno_primitives::range::CharIdx;

/// Doc-derived line content slice.
#[derive(Debug, Clone)]
pub struct LineSlice {
	pub line_idx: usize,
	pub start_char: CharIdx,
	pub end_char_incl_nl: CharIdx,
	pub content_end_char: CharIdx,
	pub start_byte: u32,
	pub text: String,
	pub has_newline: bool,
}

pub struct LineSource;

impl LineSource {
	/// Loads a line slice from a rope.
	pub fn load(rope: &Rope, line_idx: usize) -> Option<LineSlice> {
		if line_idx >= rope.len_lines() {
			return None;
		}

		let start_char = rope.line_to_char(line_idx);
		let start_byte = rope.char_to_byte(start_char) as u32;
		let line_len = rope.line(line_idx).len_chars();
		let end_char_incl_nl = start_char + line_len;

		let line_text = rope.line(line_idx);
		let mut text: String = line_text.into();
		let has_newline = text.ends_with('\n');

		let content_end_char = if has_newline {
			text.pop(); // Remove \n
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
			text,
			has_newline,
		})
	}
}
