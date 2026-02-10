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
