use super::*;

#[test]
fn test_wrap_bucket_segment_count() {
	let mut bucket = WrapBucket::new((80, 4));

	// Empty bucket returns 0
	assert_eq!(bucket.segment_count(0), 0);

	// Add an entry
	bucket.set_entry(
		0,
		WrapEntry {
			doc_version_built_at: 1,
			segments: vec![
				WrappedSegment {
					start_char_offset: 0,
					char_len: 10,
					indent_cols: 0,
				},
				WrappedSegment {
					start_char_offset: 10,
					char_len: 10,
					indent_cols: 4,
				},
			],
		},
	);

	assert_eq!(bucket.segment_count(0), 2);
	assert_eq!(bucket.segment_count(1), 0); // Non-existent line
}

#[test]
fn test_wrap_bucket_version_check() {
	let mut bucket = WrapBucket::new((80, 4));

	bucket.set_entry(
		0,
		WrapEntry {
			doc_version_built_at: 1,
			segments: vec![WrappedSegment {
				start_char_offset: 0,
				char_len: 10,
				indent_cols: 0,
			}],
		},
	);

	// Same version returns segments
	assert!(bucket.get_segments(0, 1).is_some());

	// Different version returns None (needs rebuild)
	assert!(bucket.get_segments(0, 2).is_none());
}

#[test]
fn test_wrap_buckets_lru_eviction() {
	let mut cache = WrapBuckets::with_capacity(2);
	let doc_id = DocumentId(1);

	// Add first bucket
	cache.get_or_build(doc_id, (80, 4));

	// Add second bucket
	cache.get_or_build(doc_id, (100, 4));

	// Both should exist
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&(80, 4)));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&(100, 4)));

	// Add third bucket - should evict first (LRU)
	cache.get_or_build(doc_id, (120, 4));

	// First should be evicted
	assert!(!cache.index.get(&doc_id).unwrap().contains_key(&(80, 4)));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&(100, 4)));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&(120, 4)));
}

#[test]
fn test_wrap_buckets_mru_order() {
	let mut cache = WrapBuckets::with_capacity(2);
	let doc_id = DocumentId(1);

	// Add buckets
	cache.get_or_build(doc_id, (80, 4));
	cache.get_or_build(doc_id, (100, 4));

	// Touch first bucket (make it MRU)
	cache.get_or_build(doc_id, (80, 4));

	// Add third bucket - should evict second (now LRU)
	cache.get_or_build(doc_id, (120, 4));

	assert!(cache.index.get(&doc_id).unwrap().contains_key(&(80, 4)));
	assert!(!cache.index.get(&doc_id).unwrap().contains_key(&(100, 4)));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&(120, 4)));
}

#[test]
fn test_wrap_buckets_invalidate_document() {
	let mut cache = WrapBuckets::with_capacity(4);
	let doc1 = DocumentId(1);
	let doc2 = DocumentId(2);

	cache.get_or_build(doc1, (80, 4));
	cache.get_or_build(doc1, (100, 4));
	cache.get_or_build(doc2, (80, 4));

	assert_eq!(cache.index.len(), 2);

	cache.invalidate_document(doc1);

	assert!(!cache.index.contains_key(&doc1));
	assert!(cache.index.contains_key(&doc2));
}

#[test]
fn test_wrap_buckets_invalidate_dead_slots_no_panic() {
	// Test that invalidating a document doesn't cause a panic on next insert when at capacity
	let mut cache = WrapBuckets::with_capacity(2);
	let doc1 = DocumentId(1);
	let doc2 = DocumentId(2);

	cache.get_or_build(doc1, (80, 4));
	cache.get_or_build(doc2, (80, 4));

	// Cache is now at capacity.
	// Invalidate doc1 - this removes it from index but NOT from buckets/mru_order (in fixed version)
	cache.invalidate_document(doc1);

	// Now add doc3. This should trigger eviction of the LRU slot (which was doc1's)
	let doc3 = DocumentId(3);
	let _ = cache.get_or_build(doc3, (80, 4));

	// If we get here without panic, the fix is working.
	assert!(cache.index.contains_key(&doc3));
	assert!(cache.index.contains_key(&doc2));
	assert!(!cache.index.contains_key(&doc1));
}
