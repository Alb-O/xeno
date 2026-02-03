use super::helpers::*;

#[test]
fn test_metadata_consistency() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	let docs = vec![
		(1u128, "short doc"),
		(2u128, "this is a much longer document with many more words"),
		(3u128, "medium length document"),
	];

	for (doc_id, doc) in &docs {
		bm25.insert_doc(&mut wtxn, *doc_id, doc).unwrap();
	}

	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap().unwrap();
	let metadata: BM25Metadata = postcard::from_bytes(metadata_bytes).unwrap();

	assert_eq!(metadata.total_docs, 3);
	assert!(metadata.avgdl > 0.0);
	assert_eq!(metadata.k1, 1.2);
	assert_eq!(metadata.b, 0.75);

	bm25.delete_doc(&mut wtxn, 2u128).unwrap();

	// check updated metadata
	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap().unwrap();
	let updated_metadata: BM25Metadata = postcard::from_bytes(metadata_bytes).unwrap();

	assert_eq!(updated_metadata.total_docs, 2);
	// average document length should be recalculated
	assert_ne!(updated_metadata.avgdl, metadata.avgdl);

	wtxn.commit().unwrap();
}

#[test]
fn test_insert_first_document_initializes_metadata() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	// Before inserting, metadata should not exist
	let metadata_before = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap();
	assert!(metadata_before.is_none());

	// Insert first document
	bm25.insert_doc(&mut wtxn, 1u128, "first document").unwrap();

	// After inserting, metadata should exist
	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap().unwrap();
	let metadata: BM25Metadata = postcard::from_bytes(metadata_bytes).unwrap();

	assert_eq!(metadata.total_docs, 1);
	assert!(metadata.avgdl > 0.0);

	wtxn.commit().unwrap();
}

#[test]
fn test_bm25_temp_config() {
	let (env, _temp_dir) = setup_test_env();
	let mut wtxn = env.write_txn().unwrap();

	// Create temp BM25 config with unique ID
	let config = HBM25Config::new_temp(&env, &mut wtxn, "test_unique_id").unwrap();

	// Should be able to insert and search
	config
		.insert_doc(&mut wtxn, 1u128, "test document")
		.unwrap();
	wtxn.commit().unwrap();

	let rtxn = env.read_txn().unwrap();
	let arena = Bump::new();
	let results = config.search(&rtxn, "test", 10, &arena).unwrap();
	assert_eq!(results.len(), 1);
}
