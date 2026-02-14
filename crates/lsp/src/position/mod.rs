//! Position conversion utilities for LSP.
//!
//! LSP uses `Position` (line, character) where the character offset depends on
//! the negotiated encoding (UTF-8, UTF-16, or UTF-32). This module provides
//! utilities for converting between rope character indices and LSP positions.
//!
//! # Offset Encoding
//!
//! * UTF-8: Character offset is the byte offset within the line.
//! * UTF-16: Character offset is the number of UTF-16 code units (default LSP encoding).
//! * UTF-32: Character offset is the number of Unicode codepoints (same as Rope char offset).
//!
//! Since Rope uses Unicode codepoints internally, UTF-32 is a 1:1 mapping.
//! UTF-16 requires special handling for characters outside the BMP (emoji, etc.)
//! which are represented as surrogate pairs (2 code units).

use lsp_types::{Position, Range};
use ropey::{Rope, RopeSlice};

use crate::client::OffsetEncoding;

/// Convert an LSP Position to a character index in the rope.
///
/// Returns `None` if the position is out of bounds.
pub fn lsp_position_to_char(text: &Rope, pos: Position, encoding: OffsetEncoding) -> Option<usize> {
	let line = pos.line as usize;
	if line >= text.len_lines() {
		return None;
	}

	let line_start = text.line_to_char(line);
	let line_text = text.line(line);
	let char_offset = lsp_col_to_char_offset(line_text, pos.character, encoding)?;

	Some(line_start + char_offset)
}

/// Convert a character index in the rope to an LSP Position.
///
/// Returns `None` if the index is out of bounds.
pub fn char_to_lsp_position(text: &Rope, char_idx: usize, encoding: OffsetEncoding) -> Option<Position> {
	if char_idx > text.len_chars() {
		return None;
	}

	let line = text.char_to_line(char_idx);
	let line_start = text.line_to_char(line);
	let char_offset = char_idx - line_start;
	let line_text = text.line(line);
	let lsp_col = char_offset_to_lsp_col(line_text, char_offset, encoding);

	Some(Position {
		line: line as u32,
		character: lsp_col,
	})
}

/// Convert an LSP Range to a character range (start, end).
pub fn lsp_range_to_char_range(text: &Rope, range: Range, encoding: OffsetEncoding) -> Option<(usize, usize)> {
	let start = lsp_position_to_char(text, range.start, encoding)?;
	let end = lsp_position_to_char(text, range.end, encoding)?;
	Some((start, end))
}

/// Convert a character range to an LSP Range.
pub fn char_range_to_lsp_range(text: &Rope, start: usize, end: usize, encoding: OffsetEncoding) -> Option<Range> {
	let start_pos = char_to_lsp_position(text, start, encoding)?;
	let end_pos = char_to_lsp_position(text, end, encoding)?;
	Some(Range {
		start: start_pos,
		end: end_pos,
	})
}

/// Convert an LSP character column to a rope character offset within a line.
fn lsp_col_to_char_offset(line: RopeSlice, lsp_col: u32, encoding: OffsetEncoding) -> Option<usize> {
	match encoding {
		OffsetEncoding::Utf32 => {
			// UTF-32: LSP col == char offset
			let col = lsp_col as usize;
			// Clamp to line length (excluding newline if present)
			let line_len = line_char_len_without_newline(line);
			Some(col.min(line_len))
		}
		OffsetEncoding::Utf8 => {
			// UTF-8: LSP col is byte offset
			let target_bytes = lsp_col as usize;
			let mut byte_count = 0;
			for (char_idx, ch) in line.chars().enumerate() {
				if byte_count >= target_bytes {
					return Some(char_idx);
				}
				byte_count += ch.len_utf8();
			}
			// Past end of line, clamp to line length
			Some(line_char_len_without_newline(line))
		}
		OffsetEncoding::Utf16 => {
			// UTF-16: LSP col is number of UTF-16 code units
			let target_units = lsp_col as usize;
			let mut unit_count = 0;
			for (char_idx, ch) in line.chars().enumerate() {
				if unit_count >= target_units {
					return Some(char_idx);
				}
				unit_count += ch.len_utf16();
			}
			// Past end of line, clamp to line length
			Some(line_char_len_without_newline(line))
		}
	}
}

/// Convert a rope character offset within a line to an LSP character column.
fn char_offset_to_lsp_col(line: RopeSlice, char_offset: usize, encoding: OffsetEncoding) -> u32 {
	match encoding {
		OffsetEncoding::Utf32 => {
			// UTF-32: char offset == LSP col
			char_offset as u32
		}
		OffsetEncoding::Utf8 => {
			// UTF-8: count bytes up to char_offset
			let mut byte_count = 0;
			for (idx, ch) in line.chars().enumerate() {
				if idx >= char_offset {
					break;
				}
				byte_count += ch.len_utf8();
			}
			byte_count as u32
		}
		OffsetEncoding::Utf16 => {
			// UTF-16: count code units up to char_offset
			let mut unit_count = 0;
			for (idx, ch) in line.chars().enumerate() {
				if idx >= char_offset {
					break;
				}
				unit_count += ch.len_utf16();
			}
			unit_count as u32
		}
	}
}

/// Get the character length of a line, excluding the trailing newline if present.
fn line_char_len_without_newline(line: RopeSlice) -> usize {
	let len = line.len_chars();
	if len > 0 && line.char(len - 1) == '\n' { len - 1 } else { len }
}

#[cfg(test)]
mod tests;
