use std::sync::Arc;

use crate::core::document::DocumentId;
use crate::render::buffer::diff::DiffLineNumbers;
use crate::render::cache::diff::DiffLineNumbersCache;

#[test]
fn get_or_build_reuses_existing_entry_for_same_version() {
	let mut cache = DiffLineNumbersCache::new();
	let doc_id = DocumentId(1);

	let first: Arc<Vec<DiffLineNumbers>> = Arc::clone(
		&cache
			.get_or_build(doc_id, 7, || vec![DiffLineNumbers::default()])
			.line_numbers,
	);
	let second: Arc<Vec<DiffLineNumbers>> = Arc::clone(
		&cache
			.get_or_build(doc_id, 7, || panic!("should not rebuild"))
			.line_numbers,
	);

	assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn get_or_build_rebuilds_for_new_version() {
	let mut cache = DiffLineNumbersCache::new();
	let doc_id = DocumentId(1);

	let first: Arc<Vec<DiffLineNumbers>> = Arc::clone(
		&cache
			.get_or_build(doc_id, 1, || vec![DiffLineNumbers::default()])
			.line_numbers,
	);
	let second: Arc<Vec<DiffLineNumbers>> = Arc::clone(
		&cache
			.get_or_build(doc_id, 2, || {
				vec![DiffLineNumbers {
					old: Some(1),
					new: None,
				}]
			})
			.line_numbers,
	);

	assert!(!Arc::ptr_eq(&first, &second));
	assert_eq!(second[0].old, Some(1));
}

#[test]
fn invalidate_document_clears_all_versions_for_doc() {
	let mut cache = DiffLineNumbersCache::new();
	let doc1 = DocumentId(1);
	let doc2 = DocumentId(2);

	let _ = cache.get_or_build(doc1, 1, || vec![DiffLineNumbers::default()]);
	let _ = cache.get_or_build(doc1, 2, || vec![DiffLineNumbers::default()]);
	let _ = cache.get_or_build(doc2, 1, || vec![DiffLineNumbers::default()]);

	cache.invalidate_document(doc1);

	let rebuilt: Arc<Vec<DiffLineNumbers>> = Arc::clone(
		&cache
			.get_or_build(doc1, 1, || {
				vec![DiffLineNumbers {
					old: Some(9),
					new: Some(9),
				}]
			})
			.line_numbers,
	);
	assert_eq!(rebuilt[0].old, Some(9));

	let existing: Arc<Vec<DiffLineNumbers>> = Arc::clone(
		&cache
			.get_or_build(doc2, 1, || panic!("doc2 entry should remain"))
			.line_numbers,
	);
	assert_eq!(existing.len(), 1);
}
