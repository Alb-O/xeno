//! Cache for diff gutter line-number mappings.
//!
//! Stores per-document diff line-number vectors keyed by `(DocumentId, doc_version)`
//! to avoid rebuilding the full mapping every render.

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::document::DocumentId;
use crate::render::buffer::diff::DiffLineNumbers;

/// Cache key for diff line-number mappings.
pub type DiffLineNumbersCacheKey = (DocumentId, u64);

/// Cached diff line-number mapping for a document version.
#[derive(Debug, Clone)]
pub struct DiffLineNumbersEntry {
	/// One entry per display line in the diff document.
	pub line_numbers: Arc<Vec<DiffLineNumbers>>,
}

/// Cache for diff line-number mappings.
#[derive(Debug)]
pub struct DiffLineNumbersCache {
	entries: HashMap<DiffLineNumbersCacheKey, DiffLineNumbersEntry>,
	max_entries: usize,
}

impl DiffLineNumbersCache {
	/// Default maximum number of cached document-version entries.
	pub const DEFAULT_MAX_ENTRIES: usize = 16;

	/// Creates a new empty cache with the default capacity.
	pub fn new() -> Self {
		Self {
			entries: HashMap::new(),
			max_entries: Self::DEFAULT_MAX_ENTRIES,
		}
	}

	/// Creates a new cache with a custom capacity.
	pub fn with_capacity(max_entries: usize) -> Self {
		Self {
			entries: HashMap::new(),
			max_entries,
		}
	}

	/// Returns a cached mapping, or builds and stores it if missing.
	pub fn get_or_build<F>(
		&mut self,
		doc_id: DocumentId,
		doc_version: u64,
		build_fn: F,
	) -> &DiffLineNumbersEntry
	where
		F: FnOnce() -> Vec<DiffLineNumbers>,
	{
		let key = (doc_id, doc_version);
		if self.entries.contains_key(&key) {
			return self.entries.get(&key).expect("cache entry exists");
		}

		self.enforce_capacity();
		let entry = DiffLineNumbersEntry {
			line_numbers: Arc::new(build_fn()),
		};
		self.entries.insert(key, entry);
		self.entries.get(&key).expect("cache entry inserted")
	}

	/// Invalidates all entries for a document.
	pub fn invalidate_document(&mut self, doc_id: DocumentId) {
		self.entries.retain(|(id, _), _| *id != doc_id);
	}

	fn enforce_capacity(&mut self) {
		if self.entries.len() >= self.max_entries
			&& let Some(key) = self.entries.keys().next().copied()
		{
			self.entries.remove(&key);
		}
	}
}

impl Default for DiffLineNumbersCache {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests;
