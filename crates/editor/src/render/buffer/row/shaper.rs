use xeno_primitives::Rope;
use xeno_primitives::range::CharIdx;

use super::super::plan::LineSlice;
use crate::render::wrap::{WrappedSegment, cell_width};

/// Classification of visual cells for overlay and layout logic.
///
/// Distinguishes between cells that represent real document characters and those
/// generated for visual formatting (like tab expansion or soft-wrap indentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphVirtual {
	/// A cell corresponding to a unique document character position.
	None,
	/// An auxiliary cell generated for a multi-column character.
	///
	/// Used for the 2nd and subsequent columns of a tab. These cells inherit the
	/// document position and metadata of the leading cell but are marked as fill
	/// to allow overlays (selection, cursor) to expand across the full width.
	Fill,
	/// A synthetic cell for UI or layout purposes with no document counterpart.
	///
	/// Used for continuation indents on soft-wrapped lines. These cells do not
	/// trigger syntax highlight lookups or diagnostic overlays but may participate
	/// in selection continuity.
	Layout,
}

/// A single display cell (glyph) in the rendered output.
///
/// Each glyph represents one display column. Multi-column characters like tabs
/// are expanded into multiple glyphs sharing the same document metadata.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
	/// Document character index.
	pub doc_char: CharIdx,
	/// Character offset within the line.
	pub line_char_off: usize,
	/// Document byte offset.
	pub doc_byte: u32,
	/// Display character (typically ' ' for virtual fill/layout).
	pub ch: char,
	/// Display width in columns.
	pub width: usize,
	/// Internal classification for overlay and rendering logic.
	pub virtual_kind: GlyphVirtual,
	/// Indicates if this is the first (or only) cell representing a document character.
	pub is_leading: bool,
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
			current_col: 0,
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
				virtual_kind: GlyphVirtual::Layout,
				is_leading: false,
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
				virtual_kind: GlyphVirtual::Fill,
				is_leading: false,
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
				virtual_kind: GlyphVirtual::None,
				is_leading: true,
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
			virtual_kind: GlyphVirtual::None,
			is_leading: true,
		};

		self.current_char_idx += 1;
		self.current_byte_off += char_len_utf8 as u32;
		self.current_col += char_width;

		Some(glyph)
	}
}
