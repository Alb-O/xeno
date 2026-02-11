//! Wrap cache implementation with manual LRU eviction.
//!
//! Provides [`WrapBuckets`] - a cache for line wrapping results keyed by
//! document ID and wrap configuration (text_width, tab_width).

use std::collections::{HashMap, VecDeque};

use xeno_primitives::Rope;

use crate::core::document::DocumentId;
use crate::render::buffer::plan::WrapAccess;
use crate::render::wrap::{WrappedSegment, wrap_line_ranges_rope};

/// Key for identifying a wrap bucket configuration.
pub type WrapBucketKey = (usize, usize); // (text_width, tab_width)

/// Entry for a single wrapped line.
#[derive(Debug, Clone)]
pub struct WrapEntry {
	/// Document version when this entry was built.
	pub doc_version_built_at: u64,
	/// The wrapped segments for this line.
	pub segments: Vec<WrappedSegment>,
}

/// A bucket of wrapped lines for a specific configuration.
#[derive(Debug)]
pub struct WrapBucket {
	/// The key identifying this bucket's configuration.
	pub key: WrapBucketKey,
	/// Per-line wrap entries. Index is line index.
	pub lines: Vec<Option<WrapEntry>>,
}

impl WrapBucket {
	/// Creates a new empty wrap bucket.
	pub fn new(key: WrapBucketKey) -> Self {
		Self { key, lines: Vec::new() }
	}
}

impl WrapAccess for WrapBucket {
	/// Returns the number of segments for a line.
	fn segment_count(&self, line_idx: usize) -> usize {
		self.lines.get(line_idx).and_then(|e| e.as_ref()).map(|e| e.segments.len()).unwrap_or(0)
	}
}

impl WrapAccess for &WrapBucket {
	fn segment_count(&self, line_idx: usize) -> usize {
		(*self).segment_count(line_idx)
	}
}

impl WrapAccess for &mut WrapBucket {
	fn segment_count(&self, line_idx: usize) -> usize {
		(**self).segment_count(line_idx)
	}
}

impl WrapBucket {
	/// Returns the segments for a line if cached and valid.
	pub fn get_segments(&self, line_idx: usize, doc_version: u64) -> Option<&[WrappedSegment]> {
		self.lines.get(line_idx)?.as_ref().and_then(|e| {
			if e.doc_version_built_at == doc_version {
				Some(e.segments.as_slice())
			} else {
				None
			}
		})
	}

	/// Ensures the lines vector can hold the given line index.
	pub fn ensure_capacity(&mut self, line_idx: usize) {
		if line_idx >= self.lines.len() {
			self.lines.resize_with(line_idx + 1, || None);
		}
	}

	/// Sets the wrap entry for a line.
	pub fn set_entry(&mut self, line_idx: usize, entry: WrapEntry) {
		self.ensure_capacity(line_idx);
		self.lines[line_idx] = Some(entry);
	}
}

/// Manual LRU cache for wrap buckets.
///
/// Stores up to 8 buckets of wrapped lines, evicting the least-recently-used
/// bucket when at capacity. Uses a stable-index approach to avoid frequent
/// reallocations.
#[derive(Debug)]
pub struct WrapBuckets {
	/// Storage for buckets. Indices are stable and reused after eviction.
	buckets: Vec<WrapBucket>,
	/// MRU order - front is most recently used, back is least recently used.
	/// Contains indices into `buckets`.
	mru_order: VecDeque<usize>,
	max_buckets: usize,
	/// Map from document_id -> key -> bucket index for O(1) lookup.
	index: HashMap<DocumentId, HashMap<WrapBucketKey, usize>>,
}

impl WrapBuckets {
	/// Creates a new wrap buckets cache with the default max size (8).
	pub fn new() -> Self {
		Self::with_capacity(8)
	}

	/// Creates a new wrap buckets cache with a specific capacity.
	pub fn with_capacity(max_buckets: usize) -> Self {
		Self {
			buckets: Vec::with_capacity(max_buckets),
			mru_order: VecDeque::with_capacity(max_buckets),
			max_buckets,
			index: HashMap::new(),
		}
	}

	/// Gets or builds a wrap bucket for the given document and configuration.
	///
	/// Returns an existing bucket if one matches the key and is valid. If no
	/// match is found, a new bucket is initialized, potentially evicting
	/// the least-recently-used bucket if the cache is at capacity.
	pub fn get_or_build(&mut self, doc_id: DocumentId, key: WrapBucketKey) -> &mut WrapBucket {
		if let Some(&bucket_idx) = self.index.get(&doc_id).and_then(|m| m.get(&key)) {
			self.touch(bucket_idx);
			return &mut self.buckets[bucket_idx];
		}

		let bucket_idx = if self.buckets.len() < self.max_buckets {
			let idx = self.buckets.len();
			self.buckets.push(WrapBucket::new(key));
			idx
		} else {
			let lru_idx = self.mru_order.pop_back().expect("MRU order not empty");

			if let Some(old_bucket) = self.buckets.get(lru_idx) {
				let old_key = old_bucket.key;
				self.index.retain(|_, m| {
					m.retain(|k, idx| *idx != lru_idx || *k != old_key);
					!m.is_empty()
				});
			}

			self.buckets[lru_idx] = WrapBucket::new(key);
			lru_idx
		};

		self.index.entry(doc_id).or_default().insert(key, bucket_idx);

		self.mru_order.push_front(bucket_idx);

		&mut self.buckets[bucket_idx]
	}

	fn touch(&mut self, bucket_idx: usize) {
		if let Some(pos) = self.mru_order.iter().position(|&idx| idx == bucket_idx) {
			self.mru_order.remove(pos);
		}
		self.mru_order.push_front(bucket_idx);
	}

	/// Invalidates all buckets for a document.
	///
	/// Reclaims memory by clearing cached line data for the specified document.
	/// The invalidated buckets remain in the LRU tracking and will be reused
	/// as they become the least-recently-used.
	pub fn invalidate_document(&mut self, doc_id: DocumentId) {
		if let Some(removed) = self.index.remove(&doc_id) {
			for (_key, bucket_idx) in removed {
				if let Some(bucket) = self.buckets.get_mut(bucket_idx) {
					bucket.lines.clear();
				}
			}
		}
	}

	/// Builds wrap entries for lines in the given range.
	///
	/// This populates the cache by wrapping lines from the rope.
	pub fn build_range(&mut self, doc_id: DocumentId, key: WrapBucketKey, rope: &Rope, doc_version: u64, start_line: usize, end_line: usize) {
		let bucket = self.get_or_build(doc_id, key);

		for line_idx in start_line..end_line.min(rope.len_lines()) {
			// Skip if already cached with current version
			if let Some(entry) = bucket.lines.get(line_idx).and_then(|e| e.as_ref())
				&& entry.doc_version_built_at == doc_version
			{
				continue;
			}

			// Build the wrap entry
			let line = rope.line(line_idx);
			let line_len = line.len_chars();
			let has_newline = line_len > 0 && line.char(line_len - 1) == '\n';
			let content = if has_newline { line.slice(..line_len - 1) } else { line };

			let segments = wrap_line_ranges_rope(content, key.0, key.1);

			bucket.set_entry(
				line_idx,
				WrapEntry {
					doc_version_built_at: doc_version,
					segments,
				},
			);
		}
	}
}

impl Default for WrapBuckets {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests;
