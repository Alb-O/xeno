//! Diagnostics cache for efficient diagnostic rendering.
//!
//! Provides caching infrastructure for diagnostic line maps and range maps,
//! keyed by (DocumentId, diagnostics_epoch) to avoid rebuilding maps every frame.

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::document::DocumentId;
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
mod tests;
