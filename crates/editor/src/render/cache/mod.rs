//! Render cache for efficient viewport rendering.
//!
//! Provides caching infrastructure for:
//! - Line wrapping results (per document, per wrap configuration)
//! - Syntax highlighting spans (tiled caching)
//! - Diagnostics maps (line_map, range_map keyed by epoch)
//! - Future: layout calculations

mod diagnostics;
mod highlight;
mod wrap;

pub use diagnostics::{DiagnosticsCache, DiagnosticsCacheKey, DiagnosticsEntry};
pub use highlight::{HighlightKey, HighlightTile, HighlightTiles, TILE_SIZE};
pub use wrap::{WrapBucket, WrapBucketKey, WrapBuckets, WrapEntry};

use crate::buffer::DocumentId;

/// The main render cache.
///
/// Holds all cached render data indexed by document. The cache is designed
/// for short-lock usage: snapshot the rope and version, then release the
/// document lock while building/rendering.
#[derive(Debug, Default)]
pub struct RenderCache {
	/// Wrap configuration caches per document.
	pub wrap: WrapBuckets,
	/// Highlight tile caches per document.
	pub highlight: HighlightTiles,
	/// Diagnostics caches per document (keyed by epoch).
	pub diagnostics: DiagnosticsCache,
	/// Theme epoch for cache invalidation.
	pub theme_epoch: u64,
}

impl RenderCache {
	/// Creates a new empty render cache.
	pub fn new() -> Self {
		Self::default()
	}

	/// Invalidates all cached data for a document.
	///
	/// Should be called when a document is closed to reclaim memory.
	pub fn invalidate_document(&mut self, doc_id: DocumentId) {
		self.wrap.invalidate_document(doc_id);
		self.highlight.invalidate_document(doc_id);
		self.diagnostics.invalidate_document(doc_id);
	}

	/// Updates the theme epoch, invalidating the highlight cache if changed.
	pub fn set_theme_epoch(&mut self, epoch: u64) {
		if epoch != self.theme_epoch {
			self.theme_epoch = epoch;
			self.highlight.set_theme_epoch(epoch);
		}
	}

	/// Clears all cached data.
	pub fn clear(&mut self) {
		*self = Self::new();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_render_cache_new() {
		let mut cache = RenderCache::new();
		// Cache should be empty on creation
		let _ = cache.wrap.get_or_build(DocumentId(1), (80, 4));
		// If we get here without panic, cache is working
	}

	#[test]
	fn test_render_cache_invalidate() {
		let mut cache = RenderCache::new();
		let doc_id = DocumentId(1);

		// Add a bucket
		cache.wrap.get_or_build(doc_id, (80, 4));

		// Invalidate - should not panic
		cache.invalidate_document(doc_id);

		// Adding a new bucket after invalidation should work
		let bucket = cache.wrap.get_or_build(doc_id, (80, 4));
		assert_eq!(bucket.key, (80, 4));
	}

	#[test]
	fn test_render_cache_clear() {
		let mut cache = RenderCache::new();
		let doc_id = DocumentId(1);

		cache.wrap.get_or_build(doc_id, (80, 4));
		cache.wrap.get_or_build(DocumentId(2), (100, 4));

		// Clear should not panic
		cache.clear();

		// Adding buckets after clear should work
		let bucket = cache.wrap.get_or_build(doc_id, (80, 4));
		assert_eq!(bucket.key, (80, 4));
	}
}
