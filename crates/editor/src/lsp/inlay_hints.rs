//! Inlay hint cache and LSP→render conversion.
//!
//! Manages per-buffer inlay hint caches keyed by document version and visible line range.
//! Converts LSP `InlayHint` responses into render-ready `InlayHintRangeMap` structures.

use std::collections::HashMap;
use std::sync::Arc;

use unicode_width::UnicodeWidthStr;
use xeno_lsp::{OffsetEncoding, lsp_position_to_char, lsp_types};
use xeno_primitives::Rope;

use crate::buffer::ViewId;
use crate::render::{InlayHintRangeMap, InlayHintSpan};

/// Per-buffer inlay hint cache entry.
struct CacheEntry {
	/// Document revision when hints were fetched.
	doc_rev: u64,
	/// Visible line range (start, end) when hints were fetched.
	line_range: (usize, usize),
	/// Converted hints ready for rendering.
	hints: Arc<InlayHintRangeMap>,
	/// Generation counter for in-flight de-duplication.
	request_gen: u64,
}

/// Manages inlay hint caches for all open buffers.
///
/// Tracks per-buffer generation counters and in-flight request state to prevent
/// duplicate requests. The generation counter is monotonic and persists even when
/// cache entries are cleared.
pub(crate) struct InlayHintCache {
	entries: HashMap<ViewId, CacheEntry>,
	/// Monotonic generation counters per buffer (survives cache invalidation).
	gens: HashMap<ViewId, u64>,
	/// Generation of the currently in-flight request per buffer.
	in_flight: HashMap<ViewId, u64>,
}

impl InlayHintCache {
	pub fn new() -> Self {
		Self {
			entries: HashMap::new(),
			gens: HashMap::new(),
			in_flight: HashMap::new(),
		}
	}

	/// Returns cached hints for a buffer, if the cache is still valid for the given
	/// document revision and visible line range.
	pub fn get(&self, buffer_id: ViewId, doc_rev: u64, line_lo: usize, line_hi: usize) -> Option<&Arc<InlayHintRangeMap>> {
		let entry = self.entries.get(&buffer_id)?;
		if entry.doc_rev == doc_rev && entry.line_range.0 <= line_lo && entry.line_range.1 >= line_hi {
			Some(&entry.hints)
		} else {
			None
		}
	}

	/// Stores resolved hints for a buffer and clears the in-flight marker.
	pub fn insert(&mut self, buffer_id: ViewId, doc_rev: u64, line_lo: usize, line_hi: usize, generation: u64, hints: Arc<InlayHintRangeMap>) {
		self.clear_in_flight(buffer_id, generation);
		self.entries.insert(
			buffer_id,
			CacheEntry {
				doc_rev,
				line_range: (line_lo, line_hi),
				hints,
				request_gen: generation,
			},
		);
	}

	/// Invalidates a single buffer's cache.
	pub fn invalidate(&mut self, buffer_id: ViewId) {
		self.entries.remove(&buffer_id);
	}

	/// Invalidates all caches (e.g. on `workspace/inlayHint/refresh`).
	pub fn invalidate_all(&mut self) {
		self.entries.clear();
		self.in_flight.clear();
	}

	/// Returns the current generation for a buffer (for in-flight de-dupe).
	pub fn generation(&self, buffer_id: ViewId) -> u64 {
		self.gens.get(&buffer_id).copied().unwrap_or(0)
	}

	/// Bumps generation for a buffer and returns the new value.
	pub fn bump_generation(&mut self, buffer_id: ViewId) -> u64 {
		let g = self.gens.entry(buffer_id).or_insert(0);
		*g += 1;
		*g
	}

	/// Returns true if a request with this generation is already in flight.
	pub fn is_in_flight(&self, buffer_id: ViewId) -> bool {
		self.in_flight.contains_key(&buffer_id)
	}

	/// Marks a generation as in-flight for a buffer.
	pub fn mark_in_flight(&mut self, buffer_id: ViewId, generation: u64) {
		self.in_flight.insert(buffer_id, generation);
	}

	/// Clears the in-flight marker if it matches the given generation.
	pub fn clear_in_flight(&mut self, buffer_id: ViewId, generation: u64) {
		if self.in_flight.get(&buffer_id) == Some(&generation) {
			self.in_flight.remove(&buffer_id);
		}
	}
}

/// Converts LSP inlay hints into a render-ready map, keyed by line number.
///
/// Uses encoding-aware position conversion to correctly map LSP positions
/// (which may be UTF-16 encoded) to character offsets in the document rope.
/// Hints with positions that fall outside the document are silently skipped.
pub(crate) fn convert_lsp_hints(hints: &[lsp_types::InlayHint], rope: &Rope, encoding: OffsetEncoding) -> InlayHintRangeMap {
	let mut map: InlayHintRangeMap = HashMap::new();
	for hint in hints {
		let line = hint.position.line as usize;
		let Some(abs_char) = lsp_position_to_char(rope, hint.position, encoding) else {
			continue;
		};
		let line_start = rope.line_to_char(line);
		let pos_char = abs_char - line_start;

		let label_text = match &hint.label {
			lsp_types::InlayHintLabel::String(s) => s.clone(),
			lsp_types::InlayHintLabel::LabelParts(parts) => parts.iter().map(|p| p.value.as_str()).collect::<String>(),
		};

		let cols = UnicodeWidthStr::width(label_text.as_str()) as u16;
		let kind = match hint.kind {
			Some(lsp_types::InlayHintKind::TYPE) => 1,
			Some(lsp_types::InlayHintKind::PARAMETER) => 2,
			_ => 0,
		};

		map.entry(line).or_default().push(InlayHintSpan {
			pos_char,
			text: Arc::from(label_text),
			cols,
			pad_left: hint.padding_left.unwrap_or(false),
			pad_right: hint.padding_right.unwrap_or(false),
			kind,
		});
	}

	// Sort spans within each line by position for correct rendering order.
	for spans in map.values_mut() {
		spans.sort_by_key(|s| s.pos_char);
	}

	map
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Verifies that UTF-16 encoded positions (emoji in source) are correctly
	/// converted to character offsets when the server uses UTF-16 encoding.
	///
	/// "let 🎉 = 42;" — the emoji is 1 char but 2 UTF-16 code units.
	/// A hint at UTF-16 offset 11 (after "42") should map to char offset 9.
	#[test]
	fn convert_lsp_hints_utf16_emoji() {
		// "let 🎉 = 42;\n"
		// chars: l(0) e(1) t(2) ' '(3) 🎉(4) ' '(5) =(6) ' '(7) 4(8) 2(9) ;(10) \n(11)
		// UTF-16: l(0) e(1) t(2) ' '(3) 🎉(4,5) ' '(6) =(7) ' '(8) 4(9) 2(10) ;(11) \n(12)
		let rope = Rope::from("let 🎉 = 42;\n");

		let hints = vec![lsp_types::InlayHint {
			position: lsp_types::Position { line: 0, character: 11 }, // UTF-16 offset for ";"
			label: lsp_types::InlayHintLabel::String(": i32".into()),
			kind: Some(lsp_types::InlayHintKind::TYPE),
			text_edits: None,
			tooltip: None,
			padding_left: Some(true),
			padding_right: None,
			data: None,
		}];

		let map = convert_lsp_hints(&hints, &rope, OffsetEncoding::Utf16);
		let spans = map.get(&0).expect("should have spans on line 0");
		assert_eq!(spans.len(), 1);
		// UTF-16 offset 11 = char 10 (the ";")
		assert_eq!(spans[0].pos_char, 10);
		assert_eq!(&*spans[0].text, ": i32");
		assert_eq!(spans[0].kind, 1); // TYPE
	}

	/// Verifies that UTF-8 encoded positions work correctly (no offset shift).
	#[test]
	fn convert_lsp_hints_utf8_basic() {
		let rope = Rope::from("let x = 42;\n");

		let hints = vec![lsp_types::InlayHint {
			position: lsp_types::Position { line: 0, character: 4 },
			label: lsp_types::InlayHintLabel::String(": i32".into()),
			kind: Some(lsp_types::InlayHintKind::TYPE),
			text_edits: None,
			tooltip: None,
			padding_left: None,
			padding_right: None,
			data: None,
		}];

		let map = convert_lsp_hints(&hints, &rope, OffsetEncoding::Utf8);
		let spans = map.get(&0).expect("should have spans on line 0");
		assert_eq!(spans.len(), 1);
		assert_eq!(spans[0].pos_char, 4);
	}

	/// Verifies that out-of-bounds hint positions are silently skipped.
	#[test]
	fn convert_lsp_hints_out_of_bounds_skipped() {
		let rope = Rope::from("hi\n");

		let hints = vec![lsp_types::InlayHint {
			position: lsp_types::Position { line: 5, character: 0 }, // Line 5 doesn't exist
			label: lsp_types::InlayHintLabel::String("ghost".into()),
			kind: None,
			text_edits: None,
			tooltip: None,
			padding_left: None,
			padding_right: None,
			data: None,
		}];

		let map = convert_lsp_hints(&hints, &rope, OffsetEncoding::Utf16);
		assert!(map.is_empty());
	}
}
