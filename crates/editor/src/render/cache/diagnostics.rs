//! Diagnostics cache for efficient diagnostic rendering.
//!
//! Provides caching infrastructure for diagnostic line maps and range maps,
//! keyed by (DocumentId, diagnostics_epoch) to avoid rebuilding maps every frame.

use std::collections::HashMap;
use std::sync::Arc;

use crate::buffer::DocumentId;
use crate::render::{DiagnosticLineMap, DiagnosticRangeMap};

/// Cache key for diagnostics entries.
pub type DiagnosticsCacheKey = (DocumentId, u64);

/// A cached diagnostics entry containing pre-built maps.
#[derive(Debug, Clone)]
pub struct DiagnosticsEntry {
	/// Map from line number to diagnostic severity (gutter format).
	pub line_map: Arc<DiagnosticLineMap>,
	/// Map from line number to diagnostic spans on that line.
	pub range_map: Arc<DiagnosticRangeMap>,
}

/// Cache for diagnostic maps.
///
/// Stores derived line and range maps keyed by `(DocumentId, diagnostics_epoch)`.
/// The epoch increments when diagnostics for a document change, ensuring
/// derived maps are rebuilt only when necessary.
#[derive(Debug)]
pub struct DiagnosticsCache {
	/// Cached diagnostic data keyed by (DocumentId, diag_epoch).
	entries: HashMap<DiagnosticsCacheKey, DiagnosticsEntry>,
	max_entries: usize,
}

impl Default for DiagnosticsCache {
	fn default() -> Self {
		Self::new()
	}
}

impl DiagnosticsCache {
	/// Default maximum number of cached entries across all documents.
	pub const DEFAULT_MAX_ENTRIES: usize = 32;

	/// Creates a new empty diagnostics cache with the default capacity (32).
	pub fn new() -> Self {
		Self {
			entries: HashMap::new(),
			max_entries: Self::DEFAULT_MAX_ENTRIES,
		}
	}

	/// Creates a new cache with a custom max entries limit.
	pub fn with_capacity(max_entries: usize) -> Self {
		Self {
			entries: HashMap::new(),
			max_entries,
		}
	}

	/// Gets or builds a diagnostics entry for the given document and epoch.
	///
	/// Returns a cached entry if the epoch matches. Otherwise, executes the
	/// provided closure to build new maps and caches the result.
	pub fn get_or_build<F>(
		&mut self,
		doc_id: DocumentId,
		epoch: u64,
		build_fn: F,
	) -> &DiagnosticsEntry
	where
		F: FnOnce() -> (DiagnosticLineMap, DiagnosticRangeMap),
	{
		let key = (doc_id, epoch);

		if self.entries.contains_key(&key) {
			return self.entries.get(&key).expect("just checked");
		}

		self.enforce_capacity();

		let (line_map, range_map) = build_fn();
		let entry = DiagnosticsEntry {
			line_map: Arc::new(line_map),
			range_map: Arc::new(range_map),
		};

		self.entries.insert(key, entry);
		self.entries.get(&key).expect("just inserted")
	}

	/// Gets a cached entry without building.
	pub fn get(&self, doc_id: DocumentId, epoch: u64) -> Option<&DiagnosticsEntry> {
		self.entries.get(&(doc_id, epoch))
	}

	/// Invalidates all cached data for a document.
	pub fn invalidate_document(&mut self, doc_id: DocumentId) {
		self.entries.retain(|(id, _), _| *id != doc_id);
	}

	/// Clears all cached entries.
	pub fn clear(&mut self) {
		self.entries.clear();
	}

	/// Returns the number of cached entries.
	pub fn len(&self) -> usize {
		self.entries.len()
	}

	/// Returns true if the cache is empty.
	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	/// Enforces the capacity limit by removing arbitrary entries if needed.
	///
	/// This is a simple cap, not an LRU cache, since diagnostics are relatively
	/// static and the limit is generous for typical usage patterns.
	fn enforce_capacity(&mut self) {
		if self.entries.len() >= self.max_entries {
			// Simple eviction: remove an arbitrary entry
			// Since HashMap iteration order is not guaranteed, we just remove one
			if let Some(key) = self.entries.keys().next().copied() {
				self.entries.remove(&key);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn build_test_maps() -> (DiagnosticLineMap, DiagnosticRangeMap) {
		let mut line_map = DiagnosticLineMap::new();
		line_map.insert(0, 4); // Error on line 0
		line_map.insert(5, 3); // Warning on line 5

		let mut range_map = DiagnosticRangeMap::new();
		range_map.insert(
			0,
			vec![crate::render::DiagnosticSpan {
				start_char: 0,
				end_char: 10,
				severity: 4,
			}],
		);

		(line_map, range_map)
	}

	#[test]
	fn test_cache_get_or_build_caches_entry() {
		let mut cache = DiagnosticsCache::new();
		let doc_id = DocumentId(1);
		let epoch = 42;

		// First call should build
		let entry1 = cache.get_or_build(doc_id, epoch, build_test_maps);
		assert_eq!(entry1.line_map.get(&0), Some(&4));
		assert_eq!(cache.len(), 1);

		// Second call should return cached entry
		let entry2 = cache.get_or_build(doc_id, epoch, || panic!("should not be called"));
		assert_eq!(entry2.line_map.get(&0), Some(&4));
		assert_eq!(cache.len(), 1);
	}

	#[test]
	fn test_cache_different_epochs_create_separate_entries() {
		let mut cache = DiagnosticsCache::new();
		let doc_id = DocumentId(1);

		// Build for epoch 1
		let entry1 = cache.get_or_build(doc_id, 1, build_test_maps);
		assert_eq!(entry1.line_map.get(&0), Some(&4));

		// Build for epoch 2 (different maps)
		let entry2 = cache.get_or_build(doc_id, 2, || {
			let mut line_map = DiagnosticLineMap::new();
			line_map.insert(10, 2); // Different line
			(line_map, DiagnosticRangeMap::new())
		});
		assert_eq!(entry2.line_map.get(&10), Some(&2));
		assert!(entry2.line_map.get(&0).is_none());

		// Should have 2 entries
		assert_eq!(cache.len(), 2);
	}

	#[test]
	fn test_cache_different_documents_create_separate_entries() {
		let mut cache = DiagnosticsCache::new();

		// Build for doc 1
		cache.get_or_build(DocumentId(1), 1, build_test_maps);

		// Build for doc 2
		cache.get_or_build(DocumentId(2), 1, build_test_maps);

		// Should have 2 entries
		assert_eq!(cache.len(), 2);
	}

	#[test]
	fn test_cache_capacity_limit() {
		let mut cache = DiagnosticsCache::with_capacity(3);

		// Fill to capacity
		cache.get_or_build(DocumentId(1), 1, build_test_maps);
		cache.get_or_build(DocumentId(2), 1, build_test_maps);
		cache.get_or_build(DocumentId(3), 1, build_test_maps);
		assert_eq!(cache.len(), 3);

		// Add one more - should evict
		cache.get_or_build(DocumentId(4), 1, build_test_maps);
		assert_eq!(cache.len(), 3);
	}

	#[test]
	fn test_cache_invalidate_document() {
		let mut cache = DiagnosticsCache::new();

		cache.get_or_build(DocumentId(1), 1, build_test_maps);
		cache.get_or_build(DocumentId(1), 2, build_test_maps);
		cache.get_or_build(DocumentId(2), 1, build_test_maps);
		assert_eq!(cache.len(), 3);

		// Invalidate doc 1
		cache.invalidate_document(DocumentId(1));
		assert_eq!(cache.len(), 1);
		assert!(cache.get(DocumentId(2), 1).is_some());
		assert!(cache.get(DocumentId(1), 1).is_none());
	}

	#[test]
	fn test_cache_clear() {
		let mut cache = DiagnosticsCache::new();

		cache.get_or_build(DocumentId(1), 1, build_test_maps);
		cache.get_or_build(DocumentId(2), 1, build_test_maps);
		assert_eq!(cache.len(), 2);

		cache.clear();
		assert!(cache.is_empty());
	}

	#[test]
	fn test_cache_get_returns_none_for_missing() {
		let cache = DiagnosticsCache::new();

		assert!(cache.get(DocumentId(1), 1).is_none());
	}

	#[test]
	fn test_cache_get_returns_some_for_existing() {
		let mut cache = DiagnosticsCache::new();

		cache.get_or_build(DocumentId(1), 42, build_test_maps);

		let entry = cache.get(DocumentId(1), 42);
		assert!(entry.is_some());
		assert_eq!(entry.unwrap().line_map.get(&0), Some(&4));
	}

	#[test]
	fn test_cache_epoch_mismatch_returns_none() {
		let mut cache = DiagnosticsCache::new();

		cache.get_or_build(DocumentId(1), 1, build_test_maps);

		// Different epoch should return None
		assert!(cache.get(DocumentId(1), 2).is_none());
	}
}
