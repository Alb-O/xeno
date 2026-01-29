//! Highlight tile caching for efficient syntax highlighting.
//!
//! Provides [`HighlightTiles`] - a cache for syntax highlight spans using a tiled
//! approach. Each tile covers TILE_SIZE lines (128 lines), allowing efficient
//! caching and retrieval of highlight spans for large documents.

use std::collections::{HashMap, VecDeque};

use xeno_primitives::Rope;
use xeno_runtime_language::highlight::{HighlightSpan, HighlightStyles};
use xeno_runtime_language::syntax::Syntax;
use xeno_runtime_language::{LanguageId, LanguageLoader};
use xeno_tui::style::Style;

use crate::buffer::DocumentId;

/// Number of lines per tile.
pub const TILE_SIZE: usize = 128;

/// Maximum number of tiles to cache.
const MAX_TILES: usize = 16;

/// Key for identifying a highlight tile.
///
/// The key includes all factors that affect highlight output:
/// - `syntax_version`: Changes when the syntax tree is updated
/// - `theme_epoch`: Changes when the theme is switched
/// - `language_id`: The language for syntax highlighting
/// - `tile_idx`: Which tile (line_idx / TILE_SIZE)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HighlightKey {
	/// Syntax version when this tile was built.
	pub syntax_version: u64,
	/// Theme epoch when this tile was built.
	pub theme_epoch: u64,
	/// Language ID for syntax highlighting.
	pub language_id: Option<LanguageId>,
	/// Tile index (line_idx / TILE_SIZE).
	pub tile_idx: usize,
}

/// A single cached highlight tile.
#[derive(Debug, Clone)]
pub struct HighlightTile {
	/// The key identifying this tile.
	pub key: HighlightKey,
	/// Highlight spans for this tile, with styles.
	pub spans: Vec<(HighlightSpan, Style)>,
}

/// Query parameters for retrieving highlight spans.
pub struct HighlightSpanQuery<'a, F>
where
	F: Fn(&str) -> Style,
{
	/// The document ID.
	pub doc_id: DocumentId,
	/// Current syntax version for cache validation.
	pub syntax_version: u64,
	/// The language ID for the document.
	pub language_id: Option<LanguageId>,
	/// The document content.
	pub rope: &'a Rope,
	/// The syntax tree for highlighting.
	pub syntax: &'a Syntax,
	/// The language loader.
	pub language_loader: &'a LanguageLoader,
	/// Function to resolve highlight styles.
	pub style_resolver: F,
	/// First line to highlight (inclusive).
	pub start_line: usize,
	/// Last line to highlight (exclusive).
	pub end_line: usize,
}

/// Manual LRU cache for highlight tiles.
///
/// Stores up to 16 highlight tiles covering TILE_SIZE line blocks. Evicts the
/// least-recently-used tile when at capacity. Uses a stable-index approach
/// to avoid frequent reallocations.
#[derive(Debug)]
pub struct HighlightTiles {
	/// Storage for tiles. Indices are stable and reused after eviction.
	tiles: Vec<HighlightTile>,
	/// MRU order - front is most recently used, back is least recently used.
	/// Contains indices into `tiles`.
	mru_order: VecDeque<usize>,
	max_tiles: usize,
	/// Map from document_id -> tile_idx -> tile index for O(1) lookup.
	index: HashMap<DocumentId, HashMap<usize, usize>>,
	/// Current theme epoch for cache invalidation.
	theme_epoch: u64,
}

impl HighlightTiles {
	/// Creates a new highlight tiles cache with the default max size (16).
	pub fn new() -> Self {
		Self::with_capacity(MAX_TILES)
	}

	/// Creates a new highlight tiles cache with a specific capacity.
	pub fn with_capacity(max_tiles: usize) -> Self {
		Self {
			tiles: Vec::with_capacity(max_tiles),
			mru_order: VecDeque::with_capacity(max_tiles),
			max_tiles,
			index: HashMap::new(),
			theme_epoch: 0,
		}
	}

	/// Returns the current theme epoch.
	pub fn theme_epoch(&self) -> u64 {
		self.theme_epoch
	}

	/// Updates the theme epoch, invalidating all cached tiles.
	pub fn set_theme_epoch(&mut self, epoch: u64) {
		if epoch != self.theme_epoch {
			self.theme_epoch = epoch;
			self.clear();
		}
	}

	/// Gets or builds highlight spans for the given line range.
	///
	/// Checks the cache for tiles covering the requested range. Missing or
	/// stale tiles are recomputed and cached, potentially evicting LRU tiles
	/// if the cache is at capacity.
	pub fn get_spans<F>(&mut self, q: HighlightSpanQuery<'_, F>) -> Vec<(HighlightSpan, Style)>
	where
		F: Fn(&str) -> Style,
	{
		if q.start_line >= q.end_line {
			return Vec::new();
		}

		let start_byte = q.rope.line_to_byte(q.start_line.min(q.rope.len_lines())) as u32;
		let end_byte = if q.end_line < q.rope.len_lines() {
			q.rope.line_to_byte(q.end_line) as u32
		} else {
			q.rope.len_bytes() as u32
		};

		let start_tile = q.start_line / TILE_SIZE;
		let end_tile = (q.end_line.saturating_sub(1)) / TILE_SIZE;

		let mut all_spans = Vec::new();

		for tile_idx in start_tile..=end_tile {
			let key = HighlightKey {
				syntax_version: q.syntax_version,
				theme_epoch: self.theme_epoch,
				language_id: q.language_id,
				tile_idx,
			};

			let spans = if let Some(tile) = self.get_cached_tile(q.doc_id, tile_idx, &key) {
				&tile.spans
			} else {
				let tile_start_line = tile_idx * TILE_SIZE;
				let tile_end_line = ((tile_idx + 1) * TILE_SIZE).min(q.rope.len_lines());

				let spans = self.build_tile_spans(
					q.rope,
					q.syntax,
					q.language_loader,
					&q.style_resolver,
					tile_start_line,
					tile_end_line,
				);

				let tile = HighlightTile {
					key,
					spans: spans.clone(),
				};

				self.insert_tile(q.doc_id, tile_idx, tile);
				&self.tiles[self
					.index
					.get(&q.doc_id)
					.unwrap()
					.get(&tile_idx)
					.copied()
					.unwrap()]
				.spans
			};

			// Clip spans to the requested byte range. This handles cases where tiles
			// return spans extending beyond the tile boundary or where the caller
			// only requested a sub-portion of the tiles.
			for (span, style) in spans {
				let s = span.start.max(start_byte);
				let e = span.end.min(end_byte);

				if s < e {
					all_spans.push((
						HighlightSpan {
							start: s,
							end: e,
							highlight: span.highlight,
						},
						*style,
					));
				}
			}
		}

		all_spans
	}

	fn get_cached_tile(
		&mut self,
		doc_id: DocumentId,
		tile_idx: usize,
		key: &HighlightKey,
	) -> Option<&HighlightTile> {
		let &tile_index = self.index.get(&doc_id)?.get(&tile_idx)?;

		let is_valid = {
			let tile = self.tiles.get(tile_index)?;
			tile.key == *key
		};

		if !is_valid {
			return None;
		}

		self.touch(tile_index);
		self.tiles.get(tile_index)
	}

	/// Inserts a tile into the cache, evicting LRU if necessary.
	fn insert_tile(&mut self, doc_id: DocumentId, tile_idx: usize, tile: HighlightTile) {
		if let Some(&old_index) = self.index.get(&doc_id).and_then(|m| m.get(&tile_idx))
			&& let Some(existing) = self.tiles.get_mut(old_index)
		{
			*existing = tile;
			self.touch(old_index);
			return;
		}

		let tile_index = if self.tiles.len() < self.max_tiles {
			let idx = self.tiles.len();
			self.tiles.push(tile);
			idx
		} else {
			let lru_idx = self.mru_order.pop_back().expect("MRU order not empty");

			self.index.retain(|_, m| {
				m.retain(|_, idx| *idx != lru_idx);
				!m.is_empty()
			});

			self.tiles[lru_idx] = tile;
			lru_idx
		};

		self.index
			.entry(doc_id)
			.or_default()
			.insert(tile_idx, tile_index);

		self.mru_order.push_front(tile_index);
	}

	fn touch(&mut self, tile_index: usize) {
		if let Some(pos) = self.mru_order.iter().position(|&idx| idx == tile_index) {
			self.mru_order.remove(pos);
		}
		self.mru_order.push_front(tile_index);
	}

	fn build_tile_spans<F>(
		&self,
		rope: &Rope,
		syntax: &Syntax,
		language_loader: &LanguageLoader,
		style_resolver: &F,
		start_line: usize,
		end_line: usize,
	) -> Vec<(HighlightSpan, Style)>
	where
		F: Fn(&str) -> Style,
	{
		let start_byte = rope.line_to_byte(start_line.min(rope.len_lines())) as u32;
		let end_byte = if end_line < rope.len_lines() {
			rope.line_to_byte(end_line) as u32
		} else {
			rope.len_bytes() as u32
		};

		let highlight_styles = HighlightStyles::new(
			xeno_registry::themes::SyntaxStyles::scope_names(),
			style_resolver,
		);

		let highlighter = syntax.highlighter(rope.slice(..), language_loader, start_byte..end_byte);

		highlighter
			.map(|span| {
				let style = highlight_styles.style_for_highlight(span.highlight);
				(span, style)
			})
			.collect()
	}

	/// Invalidates all cached tiles for a document.
	///
	/// Reclaims memory by removing index entries for the specified document.
	/// The invalidated tiles remain in storage and will be reused as they
	/// become the least-recently-used.
	pub fn invalidate_document(&mut self, doc_id: DocumentId) {
		if let Some(removed) = self.index.remove(&doc_id) {
			for (_tile_idx, _tile_index) in removed {
				// Slot remains in tiles and mru_order for reuse.
			}
		}
	}

	/// Clears all cached tiles.
	pub fn clear(&mut self) {
		self.tiles.clear();
		self.mru_order.clear();
		self.index.clear();
	}
}

impl Default for HighlightTiles {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_highlight_tiles_new() {
		let cache = HighlightTiles::new();
		assert_eq!(cache.theme_epoch(), 0);
		assert!(cache.tiles.is_empty());
		assert!(cache.mru_order.is_empty());
	}

	#[test]
	fn test_highlight_tiles_theme_epoch() {
		let mut cache = HighlightTiles::new();

		// Add a dummy tile
		cache.tiles.push(HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 0,
			},
			spans: vec![],
		});
		cache.mru_order.push_back(0);
		cache.index.insert(DocumentId(1), {
			let mut m = HashMap::new();
			m.insert(0, 0);
			m
		});

		// Setting a new theme epoch should clear the cache
		cache.set_theme_epoch(1);
		assert_eq!(cache.theme_epoch(), 1);
		assert!(cache.tiles.is_empty());
		assert!(cache.mru_order.is_empty());
		assert!(cache.index.is_empty());
	}

	#[test]
	fn test_highlight_tiles_lru_eviction() {
		let mut cache = HighlightTiles::with_capacity(2);
		let doc_id = DocumentId(1);

		// Insert first tile
		cache.insert_tile(
			doc_id,
			0,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 0,
				},
				spans: vec![],
			},
		);

		// Insert second tile
		cache.insert_tile(
			doc_id,
			1,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 1,
				},
				spans: vec![],
			},
		);

		// Both should exist
		assert_eq!(cache.tiles.len(), 2);
		assert!(cache.index.get(&doc_id).unwrap().contains_key(&0));
		assert!(cache.index.get(&doc_id).unwrap().contains_key(&1));

		// Insert third tile - should evict first (LRU)
		cache.insert_tile(
			doc_id,
			2,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 2,
				},
				spans: vec![],
			},
		);

		// First should be evicted
		assert_eq!(cache.tiles.len(), 2);
		assert!(!cache.index.get(&doc_id).unwrap().contains_key(&0));
		assert!(cache.index.get(&doc_id).unwrap().contains_key(&1));
		assert!(cache.index.get(&doc_id).unwrap().contains_key(&2));
	}

	#[test]
	fn test_highlight_tiles_mru_order() {
		let mut cache = HighlightTiles::with_capacity(2);
		let doc_id = DocumentId(1);

		// Insert tiles
		cache.insert_tile(
			doc_id,
			0,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 0,
				},
				spans: vec![],
			},
		);
		cache.insert_tile(
			doc_id,
			1,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 1,
				},
				spans: vec![],
			},
		);

		// Touch first tile (make it MRU)
		cache.touch(0);

		// Insert third tile - should evict second (now LRU)
		cache.insert_tile(
			doc_id,
			2,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 2,
				},
				spans: vec![],
			},
		);

		assert!(cache.index.get(&doc_id).unwrap().contains_key(&0));
		assert!(!cache.index.get(&doc_id).unwrap().contains_key(&1));
		assert!(cache.index.get(&doc_id).unwrap().contains_key(&2));
	}

	#[test]
	fn test_highlight_tiles_invalidate_document() {
		let mut cache = HighlightTiles::with_capacity(4);
		let doc1 = DocumentId(1);
		let doc2 = DocumentId(2);

		cache.insert_tile(
			doc1,
			0,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 0,
				},
				spans: vec![],
			},
		);
		cache.insert_tile(
			doc1,
			1,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 1,
				},
				spans: vec![],
			},
		);
		cache.insert_tile(
			doc2,
			0,
			HighlightTile {
				key: HighlightKey {
					syntax_version: 1,
					theme_epoch: 0,
					language_id: None,
					tile_idx: 0,
				},
				spans: vec![],
			},
		);

		assert_eq!(cache.index.len(), 2);

		cache.invalidate_document(doc1);

		assert!(!cache.index.contains_key(&doc1));
		assert!(cache.index.contains_key(&doc2));
	}

	#[test]
	fn test_highlight_tiles_key_mismatch() {
		let mut cache = HighlightTiles::new();
		let doc_id = DocumentId(1);

		let old_key = HighlightKey {
			syntax_version: 1,
			theme_epoch: 0,
			language_id: None,
			tile_idx: 0,
		};

		let new_key = HighlightKey {
			syntax_version: 2, // Different!
			theme_epoch: 0,
			language_id: None,
			tile_idx: 0,
		};

		// Insert tile with old key
		cache.insert_tile(
			doc_id,
			0,
			HighlightTile {
				key: old_key,
				spans: vec![],
			},
		);

		// Getting with new key should return None (cache miss)
		assert!(cache.get_cached_tile(doc_id, 0, &new_key).is_none());

		// Getting with old key should return Some
		assert!(cache.get_cached_tile(doc_id, 0, &old_key).is_some());
	}

	#[test]
	fn test_highlight_tiles_capacity_1_reuse_no_panic() {
		let mut cache = HighlightTiles::with_capacity(1);
		let doc_id = DocumentId(1);
		let key = HighlightKey {
			syntax_version: 1,
			theme_epoch: 0,
			language_id: None,
			tile_idx: 0,
		};

		// Insert first
		cache.insert_tile(doc_id, 0, HighlightTile { key, spans: vec![] });
		assert_eq!(cache.tiles.len(), 1);
		assert_eq!(cache.mru_order.len(), 1);

		// Insert again (reuse) - should not panic even with capacity 1
		cache.insert_tile(doc_id, 0, HighlightTile { key, spans: vec![] });
		assert_eq!(cache.tiles.len(), 1);
		assert_eq!(cache.mru_order.len(), 1);
	}
}
