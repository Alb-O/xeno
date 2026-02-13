//! Highlight tile caching for efficient syntax highlighting.
//!
//! Provides [`HighlightTiles`] - a cache for syntax highlight spans using a tiled
//! approach. Each tile covers TILE_SIZE lines (128 lines), allowing efficient
//! caching and retrieval of highlight spans for large documents.
//!
//! For stale-tree rendering, projected tiles are built by first mapping each
//! target tile window back to the corresponding source tile range through the
//! pending changeset, then remapping source spans forward. This avoids the
//! previous same-index assumption, which could leave persistent unstyled regions
//! after large undo/redo edits. If projection produces no spans for the request
//! window, rendering falls back to non-projected stale spans as a safety net.

mod builder;

use std::collections::{HashMap, VecDeque};

use builder::line_to_byte_or_eof;
#[cfg(test)]
use builder::remap_stale_span_to_current;
use tracing::trace;
use xeno_language::highlight::HighlightSpan;
use xeno_language::syntax::Syntax;
use xeno_language::{LanguageId, LanguageLoader};
use xeno_primitives::transaction::Bias;
use xeno_primitives::{Rope, Style};

use crate::core::document::DocumentId;
use crate::syntax_manager::HighlightProjectionCtx;

/// Number of lines per tile.
pub const TILE_SIZE: usize = 128;

/// Maximum number of tiles to cache.
const MAX_TILES: usize = 16;
/// Maximum number of projected tiles cached for stale-tree rendering.
const MAX_PROJECTED_TILES: usize = 24;

/// Key for identifying a highlight tile.
///
/// The key includes all factors that affect highlight output:
/// - `syntax_version`: Changes when the syntax tree is updated (authoritative)
/// - `theme_epoch`: Changes when the theme is switched
/// - `language_id`: The language for syntax highlighting
/// - `tile_idx`: Which tile (line_idx / TILE_SIZE)
///
/// Note: `doc_version` is NOT included here to allow reusing "best effort"
/// highlighting from a stale tree while interactive edits are in flight.
/// Correctness is maintained via `syntax_version` which identifies the
/// specific tree used to generate the spans.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProjectedHighlightKey {
	base: HighlightKey,
	target_doc_version: u64,
}

#[derive(Debug, Clone)]
struct ProjectedHighlightTile {
	key: ProjectedHighlightKey,
	spans: Vec<(HighlightSpan, Style)>,
}

/// Query parameters for retrieving highlight spans.
///
/// Groups all parameters needed for a highlight query into a single object
/// to avoid "bool soup" and provide a stable interface.
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
	/// Optional projection context for stale-tree rendering.
	pub projection: Option<HighlightProjectionCtx<'a>>,
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
	/// Projected stale-highlight tiles keyed by source key + target doc version.
	projected_tiles: Vec<ProjectedHighlightTile>,
	/// MRU order for projected tiles.
	projected_mru_order: VecDeque<usize>,
	/// Max projected tile capacity.
	max_projected_tiles: usize,
	/// O(1) lookup for projected tiles.
	projected_index: HashMap<(DocumentId, usize, u64), usize>,
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
		assert!(max_tiles > 0, "HighlightTiles capacity must be greater than 0");
		Self {
			tiles: Vec::with_capacity(max_tiles),
			mru_order: VecDeque::with_capacity(max_tiles),
			max_tiles,
			index: HashMap::new(),
			projected_tiles: Vec::with_capacity(MAX_PROJECTED_TILES),
			projected_mru_order: VecDeque::with_capacity(MAX_PROJECTED_TILES),
			max_projected_tiles: MAX_PROJECTED_TILES,
			projected_index: HashMap::new(),
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

		let start_byte = line_to_byte_or_eof(q.rope, q.start_line);
		let end_byte = if q.end_line < q.rope.len_lines() {
			q.rope.line_to_byte(q.end_line) as u32
		} else {
			q.rope.len_bytes() as u32
		};

		let mut start_tile = q.start_line / TILE_SIZE;
		let mut end_tile = (q.end_line.saturating_sub(1)) / TILE_SIZE;
		if q.projection.is_some() {
			// Pull one neighboring source tile in each direction so projected spans
			// that cross tile boundaries after remapping stay visually continuous.
			start_tile = start_tile.saturating_sub(1);
			end_tile = end_tile.saturating_add(1);
		}

		let mut all_spans = Vec::new();
		trace!(
			target: "xeno_undo_trace",
			doc_id = ?q.doc_id,
			syntax_version = q.syntax_version,
			projection = q.projection.is_some(),
			start_line = q.start_line,
			end_line = q.end_line,
			start_tile,
			end_tile,
			"render.highlight_cache.get_spans.begin"
		);

		for tile_idx in start_tile..=end_tile {
			let key = HighlightKey {
				syntax_version: q.syntax_version,
				theme_epoch: self.theme_epoch,
				language_id: q.language_id,
				tile_idx,
			};

			let spans: &[(HighlightSpan, Style)] = if let Some(projection) = q.projection {
				let projected_idx = self.get_or_build_projected_tile_index(&q, tile_idx, key, projection);
				&self.projected_tiles[projected_idx].spans
			} else {
				let tile_index = self.get_or_build_tile_index(&q, q.rope, tile_idx, key);
				&self.tiles[tile_index].spans
			};

			// Clip spans to requested range. Source/projected tiles may cover more
			// than the caller window.
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

		if q.projection.is_some() && all_spans.is_empty() {
			// Safety net for pathological remaps: prefer stale styling over
			// leaving a permanently unstyled viewport region.
			trace!(
					target: "xeno_undo_trace",
					doc_id = ?q.doc_id,
					syntax_version = q.syntax_version,
					start_line = q.start_line,
					end_line = q.end_line,
					"render.highlight_cache.get_spans.fallback_unprojected"
			);
			for tile_idx in start_tile..=end_tile {
				let key = HighlightKey {
					syntax_version: q.syntax_version,
					theme_epoch: self.theme_epoch,
					language_id: q.language_id,
					tile_idx,
				};
				let tile_index = self.get_or_build_tile_index(&q, q.rope, tile_idx, key);
				for (span, style) in &self.tiles[tile_index].spans {
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
		}

		trace!(
			target: "xeno_undo_trace",
			doc_id = ?q.doc_id,
			syntax_version = q.syntax_version,
			projection = q.projection.is_some(),
			start_line = q.start_line,
			end_line = q.end_line,
			span_count = all_spans.len(),
			"render.highlight_cache.get_spans.done"
		);
		all_spans
	}

	fn tile_byte_bounds(rope: &Rope, tile_idx: usize) -> (u32, u32) {
		let tile_start_line = tile_idx * TILE_SIZE;
		let tile_end_line = ((tile_idx + 1) * TILE_SIZE).min(rope.len_lines());
		let tile_start_byte = line_to_byte_or_eof(rope, tile_start_line);
		let tile_end_byte = if tile_end_line < rope.len_lines() {
			rope.line_to_byte(tile_end_line) as u32
		} else {
			rope.len_bytes() as u32
		};
		(tile_start_byte, tile_end_byte)
	}

	fn projection_old_char_lower_bound(projection: HighlightProjectionCtx<'_>, target_char: usize, old_len_chars: usize) -> usize {
		let mut lo = 0usize;
		let mut hi = old_len_chars;
		while lo < hi {
			let mid = lo + (hi - lo) / 2;
			let mapped = projection.composed_changes.map_pos(mid, Bias::Right);
			if mapped < target_char {
				lo = mid + 1;
			} else {
				hi = mid;
			}
		}
		lo
	}

	fn projection_old_char_upper_bound(projection: HighlightProjectionCtx<'_>, target_char: usize, old_len_chars: usize) -> usize {
		let mut lo = 0usize;
		let mut hi = old_len_chars;
		while lo < hi {
			let mid = lo + (hi - lo) / 2;
			let mapped = projection.composed_changes.map_pos(mid, Bias::Left);
			if mapped <= target_char {
				lo = mid + 1;
			} else {
				hi = mid;
			}
		}
		lo
	}

	/// Maps a target tile window back to an old-rope source tile span.
	///
	/// The changeset mapping is monotonic in character space, so we can invert
	/// target bounds via binary search over old positions.
	fn mapped_source_tile_bounds_for_target_tile(projection: HighlightProjectionCtx<'_>, target_rope: &Rope, target_tile_idx: usize) -> (usize, usize) {
		let (target_start_byte, target_end_byte) = Self::tile_byte_bounds(target_rope, target_tile_idx);
		let target_len_chars = target_rope.len_chars();
		let target_start_char = target_rope.byte_to_char(target_start_byte as usize).min(target_len_chars);
		let target_end_char = target_rope.byte_to_char(target_end_byte as usize).min(target_len_chars);

		let old_len_chars = projection.base_rope.len_chars();
		let old_start_char = Self::projection_old_char_lower_bound(projection, target_start_char, old_len_chars);
		let old_end_char = Self::projection_old_char_upper_bound(projection, target_end_char, old_len_chars);

		let old_start_byte = projection.base_rope.char_to_byte(old_start_char.min(old_len_chars));
		let old_end_byte = projection.base_rope.char_to_byte(old_end_char.min(old_len_chars));
		let old_start_line = projection.base_rope.byte_to_line(old_start_byte.min(projection.base_rope.len_bytes()));
		let old_end_line = projection.base_rope.byte_to_line(old_end_byte.min(projection.base_rope.len_bytes()));

		let mut source_start_tile = old_start_line / TILE_SIZE;
		let max_source_tile = projection.base_rope.len_lines().saturating_sub(1) / TILE_SIZE;
		let mut source_end_tile = (old_end_line / TILE_SIZE).min(max_source_tile);

		source_start_tile = source_start_tile.saturating_sub(1);
		source_end_tile = source_end_tile.saturating_add(1).min(max_source_tile);
		if source_end_tile < source_start_tile {
			source_end_tile = source_start_tile;
		}
		(source_start_tile, source_end_tile)
	}

	fn get_or_build_tile_index<F>(&mut self, q: &HighlightSpanQuery<'_, F>, rope: &Rope, tile_idx: usize, key: HighlightKey) -> usize
	where
		F: Fn(&str) -> Style,
	{
		if let Some(tile_index) = self.get_cached_tile_index(q.doc_id, tile_idx, &key) {
			return tile_index;
		}

		let tile_start_line = tile_idx * TILE_SIZE;
		let tile_end_line = ((tile_idx + 1) * TILE_SIZE).min(rope.len_lines());

		let spans = builder::build_tile_spans(rope, q.syntax, q.language_loader, &q.style_resolver, tile_start_line, tile_end_line);

		let tile = HighlightTile { key, spans };
		self.insert_tile(q.doc_id, tile_idx, tile)
	}

	fn get_or_build_projected_tile_index<F>(
		&mut self,
		q: &HighlightSpanQuery<'_, F>,
		tile_idx: usize,
		base_key: HighlightKey,
		projection: HighlightProjectionCtx<'_>,
	) -> usize
	where
		F: Fn(&str) -> Style,
	{
		let key = ProjectedHighlightKey {
			base: base_key,
			target_doc_version: projection.target_doc_version,
		};

		if let Some(tile_index) = self.get_cached_projected_tile_index(q.doc_id, tile_idx, &key) {
			return tile_index;
		}

		let (source_start_tile, source_end_tile) = Self::mapped_source_tile_bounds_for_target_tile(projection, q.rope, tile_idx);
		let mut spans = Vec::new();
		for source_tile_idx in source_start_tile..=source_end_tile {
			let source_key = HighlightKey {
				tile_idx: source_tile_idx,
				..base_key
			};
			let source_tile_index = self.get_or_build_tile_index(q, projection.base_rope, source_tile_idx, source_key);
			spans.extend(builder::project_spans_to_target(&self.tiles[source_tile_index].spans, projection, q.rope));
		}

		let (target_start_byte, target_end_byte) = Self::tile_byte_bounds(q.rope, tile_idx);
		spans.retain(|(span, _)| span.end > target_start_byte && span.start < target_end_byte);
		trace!(
			target: "xeno_undo_trace",
			doc_id = ?q.doc_id,
			syntax_version = base_key.syntax_version,
			target_tile_idx = tile_idx,
			source_start_tile,
			source_end_tile,
			target_start_byte,
			target_end_byte,
			span_count = spans.len(),
			"render.highlight_cache.projected_tile.built"
		);
		let tile = ProjectedHighlightTile { key, spans };
		self.insert_projected_tile(q.doc_id, tile_idx, projection.target_doc_version, tile)
	}

	fn get_cached_tile_index(&mut self, doc_id: DocumentId, tile_idx: usize, key: &HighlightKey) -> Option<usize> {
		let &tile_index = self.index.get(&doc_id)?.get(&tile_idx)?;

		let is_valid = {
			let tile = self.tiles.get(tile_index)?;
			tile.key == *key
		};

		if !is_valid {
			return None;
		}

		self.touch(tile_index);
		Some(tile_index)
	}

	fn get_cached_projected_tile_index(&mut self, doc_id: DocumentId, tile_idx: usize, key: &ProjectedHighlightKey) -> Option<usize> {
		let &tile_index = self.projected_index.get(&(doc_id, tile_idx, key.target_doc_version))?;

		let is_valid = {
			let tile = self.projected_tiles.get(tile_index)?;
			tile.key == *key
		};

		if !is_valid {
			return None;
		}

		self.touch_projected(tile_index);
		Some(tile_index)
	}

	#[cfg(test)]
	fn get_cached_tile(&mut self, doc_id: DocumentId, tile_idx: usize, key: &HighlightKey) -> Option<&HighlightTile> {
		let idx = self.get_cached_tile_index(doc_id, tile_idx, key)?;
		self.tiles.get(idx)
	}

	/// Inserts a tile into the cache, evicting LRU if necessary.
	/// Returns the index of the inserted tile.
	fn insert_tile(&mut self, doc_id: DocumentId, tile_idx: usize, tile: HighlightTile) -> usize {
		if let Some(&old_index) = self.index.get(&doc_id).and_then(|m| m.get(&tile_idx))
			&& let Some(existing) = self.tiles.get_mut(old_index)
		{
			*existing = tile;
			self.touch(old_index);
			return old_index;
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

		self.index.entry(doc_id).or_default().insert(tile_idx, tile_index);

		self.mru_order.push_front(tile_index);
		tile_index
	}

	fn insert_projected_tile(&mut self, doc_id: DocumentId, tile_idx: usize, target_doc_version: u64, tile: ProjectedHighlightTile) -> usize {
		let key = (doc_id, tile_idx, target_doc_version);
		if let Some(&old_index) = self.projected_index.get(&key)
			&& let Some(existing) = self.projected_tiles.get_mut(old_index)
		{
			*existing = tile;
			self.touch_projected(old_index);
			return old_index;
		}

		let tile_index = if self.projected_tiles.len() < self.max_projected_tiles {
			let idx = self.projected_tiles.len();
			self.projected_tiles.push(tile);
			idx
		} else {
			let lru_idx = self.projected_mru_order.pop_back().expect("projected MRU order not empty");

			self.projected_index.retain(|_, idx| *idx != lru_idx);
			self.projected_tiles[lru_idx] = tile;
			lru_idx
		};

		self.projected_index.insert(key, tile_index);
		self.projected_mru_order.push_front(tile_index);
		tile_index
	}

	fn touch(&mut self, tile_index: usize) {
		if let Some(pos) = self.mru_order.iter().position(|&idx| idx == tile_index) {
			self.mru_order.remove(pos);
		}
		self.mru_order.push_front(tile_index);
	}

	fn touch_projected(&mut self, tile_index: usize) {
		if let Some(pos) = self.projected_mru_order.iter().position(|&idx| idx == tile_index) {
			self.projected_mru_order.remove(pos);
		}
		self.projected_mru_order.push_front(tile_index);
	}

	/// Invalidates all cached tiles for a document.
	///
	/// Reclaims memory by removing index entries for the specified document.
	/// Corresponding tile indices are NOT removed from MRU order to preserve
	/// LRU invariants and prevent panics. They will be evicted normally.
	pub fn invalidate_document(&mut self, doc_id: DocumentId) {
		self.index.remove(&doc_id);
		self.projected_index.retain(|(id, _, _), _| *id != doc_id);
	}

	/// Clears all cached tiles.
	pub fn clear(&mut self) {
		self.tiles.clear();
		self.mru_order.clear();
		self.index.clear();
		self.projected_tiles.clear();
		self.projected_mru_order.clear();
		self.projected_index.clear();
	}
}

impl Default for HighlightTiles {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests;
