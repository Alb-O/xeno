use xeno_primitives::Rope;
use xeno_primitives::range::CharIdx;

use super::super::plan::LineSlice;
use crate::render::wrap::{WrappedSegment, cell_width};

/// A single display cell (glyph) in the rendered output.
///
/// Each glyph represents one display column. For expanded tabs and continuation
/// indents, multiple glyphs may share the same document position.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
	/// Document character index (may be shared by virtual glyphs).
	pub doc_char: CharIdx,
	/// Character offset within the line.
	pub line_char_off: usize,
	/// Document byte offset.
	pub doc_byte: u32,
	/// Display character (may be space for tab expansion).
	pub ch: char,
	/// Display width in columns (usually 1).
	pub width: usize,
	/// True for glyphs that don't correspond to actual document characters
	/// (tab expansion spaces, continuation indent).
	pub is_virtual: bool,
}

/// Iterator that converts a line segment into display glyphs.
///
/// Handles:
/// - Continuation indent (virtual spaces at start of wrapped lines)
/// - Tab expansion (variable-width, virtual spaces after first)
/// - Unicode width calculation
pub struct SegmentGlyphIter<'a> {
	line: &'a LineSlice,
	segment: &'a WrappedSegment,
	tab_width: usize,
	text_width: usize,
	chars: ropey::iter::Chars<'a>,
	current_char_idx: usize,
	current_byte_off: u32,
	current_col: usize,
	pending_tab_spaces: usize,
	tab_meta: Option<(CharIdx, usize, u32)>,
}

impl<'a> SegmentGlyphIter<'a> {
	pub fn new(
		rope: &'a Rope,
		line: &'a LineSlice,
		segment: &'a WrappedSegment,
		tab_width: usize,
		text_width: usize,
	) -> Self {
		let content_slice = line.content_slice(rope);
		let mut chars = content_slice.chars();
		let mut byte_off: u32 = 0;
		for _ in 0..segment.start_char_offset {
			if let Some(ch) = chars.next() {
				byte_off = byte_off.wrapping_add(ch.len_utf8() as u32);
			}
		}

		Self {
			line,
			segment,
			tab_width,
			text_width,
			chars,
			current_char_idx: segment.start_char_offset,
			current_byte_off: byte_off,
			current_col: segment.indent_cols,
			pending_tab_spaces: 0,
			tab_meta: None,
		}
	}
}

impl<'a> Iterator for SegmentGlyphIter<'a> {
	type Item = Glyph;

	fn next(&mut self) -> Option<Self::Item> {
		if self.current_col >= self.text_width {
			return None;
		}

		// Emit virtual spaces for continuation indent on wrapped lines.
		if self.current_col < self.segment.indent_cols && self.segment.start_char_offset > 0 {
			self.current_col += 1;
			return Some(Glyph {
				doc_char: self.line.start_char,
				line_char_off: 0,
				doc_byte: self.line.start_byte,
				ch: ' ',
				width: 1,
				is_virtual: true,
			});
		}

		// Emit remaining spaces for expanded tabs (all but first are virtual).
		if self.pending_tab_spaces > 0 {
			self.pending_tab_spaces -= 1;
			self.current_col += 1;
			let (doc_char, line_char_off, doc_byte) = self.tab_meta.unwrap();
			return Some(Glyph {
				doc_char,
				line_char_off,
				doc_byte,
				ch: ' ',
				width: 1,
				is_virtual: true,
			});
		}

		if self.current_char_idx >= self.segment.start_char_offset + self.segment.char_len {
			return None;
		}

		let ch = self.chars.next()?;
		let char_len_utf8 = ch.len_utf8();

		if ch == '\t' {
			let mut spaces = self
				.tab_width
				.saturating_sub(self.current_col % self.tab_width);
			if spaces == 0 {
				spaces = 1;
			}

			let remaining = self.text_width - self.current_col;
			spaces = spaces.min(remaining);

			let doc_char = self.line.start_char + self.current_char_idx;
			let line_char_off = self.current_char_idx;
			let doc_byte = self.line.start_byte + self.current_byte_off;

			self.current_char_idx += 1;
			self.current_byte_off += char_len_utf8 as u32;

			if spaces > 1 {
				self.pending_tab_spaces = spaces - 1;
				self.tab_meta = Some((doc_char, line_char_off, doc_byte));
			}
			self.current_col += 1;

			return Some(Glyph {
				doc_char,
				line_char_off,
				doc_byte,
				ch: ' ',
				width: 1,
				is_virtual: false,
			});
		}

		let char_width = cell_width(ch, self.current_col, self.tab_width);
		let remaining = self.text_width - self.current_col;

		if char_width > remaining {
			// Character doesn't fit, truncate row
			self.current_col = self.text_width;
			return None;
		}

		let glyph = Glyph {
			doc_char: self.line.start_char + self.current_char_idx,
			line_char_off: self.current_char_idx,
			doc_byte: self.line.start_byte + self.current_byte_off,
			ch,
			width: char_width,
			is_virtual: false,
		};

		self.current_char_idx += 1;
		self.current_byte_off += char_len_utf8 as u32;
		self.current_col += char_width;

		Some(glyph)
	}
}
