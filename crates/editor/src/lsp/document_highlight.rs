//! Document highlight cache for references-under-cursor rendering.
//!
//! Manages per-buffer document highlight caches with generation-gated
//! invalidation. Highlights are stored as byte ranges with kind information,
//! ready for post-resolution bg blending in the render pipeline.

use std::collections::HashMap;
use std::ops::Range;

use xeno_lsp::lsp_types::DocumentHighlightKind;

use crate::buffer::ViewId;

/// Decoded document highlight: byte range + kind.
pub(crate) type DocumentHighlightSpans = Vec<(Range<u32>, DocumentHighlightKind)>;

/// Number of editor ticks the cursor must remain stable before requesting highlights.
pub(crate) const DOCUMENT_HIGHLIGHT_SETTLE_TICKS: u64 = 2;

/// Per-buffer document highlight cache entry.
struct CacheEntry {
	/// Document revision when highlights were fetched.
	doc_rev: u64,
	/// Cursor position (char index) when highlights were fetched.
	cursor: usize,
	/// Decoded highlights ready for render.
	spans: DocumentHighlightSpans,
	/// Generation counter for in-flight de-duplication.
	request_gen: u64,
}

/// Manages document highlight caches for all open buffers.
///
/// Mirrors the semantic token cache pattern: generation counters for
/// de-duplication, in-flight tracking, and debounce settle counting.
pub(crate) struct DocumentHighlightCache {
	entries: HashMap<ViewId, CacheEntry>,
	/// Monotonic generation counters per buffer.
	gens: HashMap<ViewId, u64>,
	/// Generation of the currently in-flight request per buffer.
	in_flight: HashMap<ViewId, u64>,
	/// Per-buffer settle counter: incremented each tick while cursor is stable.
	settle: HashMap<ViewId, (usize, u64, u64)>, // (cursor, doc_rev, tick_count)
}

impl DocumentHighlightCache {
	pub fn new() -> Self {
		Self {
			entries: HashMap::new(),
			gens: HashMap::new(),
			in_flight: HashMap::new(),
			settle: HashMap::new(),
		}
	}

	/// Returns cached highlights for a buffer if valid for the given doc revision and cursor.
	pub fn get(&self, buffer_id: ViewId, doc_rev: u64, cursor: usize) -> Option<&DocumentHighlightSpans> {
		let entry = self.entries.get(&buffer_id)?;
		if entry.doc_rev == doc_rev && entry.cursor == cursor {
			Some(&entry.spans)
		} else {
			None
		}
	}

	/// Returns highlights for rendering with stale fallback during request churn.
	///
	/// Rendering prefers exact cursor matches. If the cursor changed, this returns
	/// the previous spans only while the cursor is still settling or a new request
	/// is in flight for the same document revision.
	pub fn get_for_render(&self, buffer_id: ViewId, doc_rev: u64, cursor: usize, settle_threshold: u64) -> Option<&DocumentHighlightSpans> {
		let entry = self.entries.get(&buffer_id)?;
		if entry.doc_rev != doc_rev {
			return None;
		}
		if entry.cursor == cursor {
			return Some(&entry.spans);
		}
		if self.is_in_flight(buffer_id) || self.is_settling(buffer_id, cursor, doc_rev, settle_threshold) {
			return Some(&entry.spans);
		}
		None
	}

	/// Stores highlights for a buffer and clears the in-flight marker.
	pub fn insert(&mut self, buffer_id: ViewId, doc_rev: u64, cursor: usize, generation: u64, spans: DocumentHighlightSpans) {
		self.clear_in_flight(buffer_id, generation);
		self.entries.insert(
			buffer_id,
			CacheEntry {
				doc_rev,
				cursor,
				spans,
				request_gen: generation,
			},
		);
	}

	/// Returns the current generation for a buffer.
	pub fn generation(&self, buffer_id: ViewId) -> u64 {
		self.gens.get(&buffer_id).copied().unwrap_or(0)
	}

	/// Bumps generation for a buffer and returns the new value.
	pub fn bump_generation(&mut self, buffer_id: ViewId) -> u64 {
		let g = self.gens.entry(buffer_id).or_insert(0);
		*g += 1;
		*g
	}

	/// Returns true if a request is already in flight for this buffer.
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

	/// Tracks cursor settle state for debouncing. Returns true when the cursor
	/// has been stable long enough to trigger a request (>= `threshold` ticks).
	pub fn tick_settle(&mut self, buffer_id: ViewId, cursor: usize, doc_rev: u64, threshold: u64) -> bool {
		let entry = self.settle.entry(buffer_id).or_insert((cursor, doc_rev, 0));
		if entry.0 != cursor || entry.1 != doc_rev {
			// Cursor moved or doc changed — reset settle counter.
			*entry = (cursor, doc_rev, 1);
			false
		} else {
			entry.2 += 1;
			entry.2 >= threshold
		}
	}

	fn is_settling(&self, buffer_id: ViewId, cursor: usize, doc_rev: u64, threshold: u64) -> bool {
		self.settle
			.get(&buffer_id)
			.is_some_and(|(settle_cursor, settle_doc_rev, ticks)| *settle_cursor == cursor && *settle_doc_rev == doc_rev && *ticks < threshold)
	}
}

/// Converts LSP document highlights to byte-range spans.
pub(crate) fn decode_document_highlights(
	highlights: &[xeno_lsp::lsp_types::DocumentHighlight],
	rope: &xeno_primitives::Rope,
	encoding: xeno_lsp::OffsetEncoding,
) -> DocumentHighlightSpans {
	let mut result = Vec::with_capacity(highlights.len());
	for hl in highlights {
		let Some(start_char) = xeno_lsp::lsp_position_to_char(rope, hl.range.start, encoding) else {
			continue;
		};
		let Some(end_char) = xeno_lsp::lsp_position_to_char(rope, hl.range.end, encoding) else {
			continue;
		};
		let start_byte = rope.char_to_byte(start_char) as u32;
		let end_byte = rope.char_to_byte(end_char) as u32;
		if start_byte >= end_byte {
			continue;
		}
		let kind = hl.kind.unwrap_or(DocumentHighlightKind::TEXT);
		result.push((start_byte..end_byte, kind));
	}
	result
}

#[cfg(test)]
mod tests {
	use xeno_lsp::lsp_types;
	use xeno_primitives::Rope;

	use super::*;

	#[test]
	fn decode_highlights_basic() {
		let rope = Rope::from("fn foo() { foo() }\n");
		let highlights = vec![
			lsp_types::DocumentHighlight {
				range: lsp_types::Range::new(lsp_types::Position::new(0, 3), lsp_types::Position::new(0, 6)),
				kind: Some(DocumentHighlightKind::WRITE),
			},
			lsp_types::DocumentHighlight {
				range: lsp_types::Range::new(lsp_types::Position::new(0, 11), lsp_types::Position::new(0, 14)),
				kind: Some(DocumentHighlightKind::READ),
			},
		];

		let spans = decode_document_highlights(&highlights, &rope, xeno_lsp::OffsetEncoding::Utf8);
		assert_eq!(spans.len(), 2);
		assert_eq!(spans[0].0, 3..6);
		assert_eq!(spans[0].1, DocumentHighlightKind::WRITE);
		assert_eq!(spans[1].0, 11..14);
		assert_eq!(spans[1].1, DocumentHighlightKind::READ);
	}

	#[test]
	fn cache_hit_miss() {
		let mut cache = DocumentHighlightCache::new();
		let id = ViewId::text(1);

		assert!(cache.get(id, 1, 5).is_none());

		cache.insert(id, 1, 5, 1, vec![(0..3, DocumentHighlightKind::TEXT)]);
		assert!(cache.get(id, 1, 5).is_some());
		// Different cursor → miss
		assert!(cache.get(id, 1, 6).is_none());
		// Different doc_rev → miss
		assert!(cache.get(id, 2, 5).is_none());
	}

	#[test]
	fn settle_debounce() {
		let mut cache = DocumentHighlightCache::new();
		let id = ViewId::text(1);

		// First tick: settle=1, threshold=2 → not ready
		assert!(!cache.tick_settle(id, 10, 1, 2));
		// Second tick same cursor: settle=2 → ready
		assert!(cache.tick_settle(id, 10, 1, 2));
		// Cursor moves: reset
		assert!(!cache.tick_settle(id, 11, 1, 2));
	}

	#[test]
	fn render_fallback_only_while_pending_or_in_flight() {
		let mut cache = DocumentHighlightCache::new();
		let id = ViewId::text(1);
		let spans = vec![(0..3, DocumentHighlightKind::TEXT)];
		cache.insert(id, 1, 5, 1, spans.clone());

		// Exact cursor: always render.
		assert_eq!(cache.get_for_render(id, 1, 5, DOCUMENT_HIGHLIGHT_SETTLE_TICKS), Some(&spans));

		// Cursor moved: first settle tick keeps previous highlights visible.
		assert!(!cache.tick_settle(id, 8, 1, DOCUMENT_HIGHLIGHT_SETTLE_TICKS));
		assert_eq!(cache.get_for_render(id, 1, 8, DOCUMENT_HIGHLIGHT_SETTLE_TICKS), Some(&spans));

		// After settle threshold, fallback stops unless a request is in flight.
		assert!(cache.tick_settle(id, 8, 1, DOCUMENT_HIGHLIGHT_SETTLE_TICKS));
		assert!(cache.get_for_render(id, 1, 8, DOCUMENT_HIGHLIGHT_SETTLE_TICKS).is_none());

		cache.mark_in_flight(id, 2);
		assert_eq!(cache.get_for_render(id, 1, 8, DOCUMENT_HIGHLIGHT_SETTLE_TICKS), Some(&spans));

		cache.clear_in_flight(id, 2);
		assert!(cache.get_for_render(id, 1, 8, DOCUMENT_HIGHLIGHT_SETTLE_TICKS).is_none());

		// Never render stale highlights across document revisions.
		assert!(cache.get_for_render(id, 2, 8, DOCUMENT_HIGHLIGHT_SETTLE_TICKS).is_none());
	}
}
