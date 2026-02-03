use super::helpers::*;

#[test]
fn test_insert_document() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	let doc_id = 123u128;
	let doc = "The quick brown fox jumps over the lazy dog";

	let result = bm25.insert_doc(&mut wtxn, doc_id, doc);
	assert!(result.is_ok());

	// check that document length was stored
	let doc_length = bm25.doc_lengths_db.get(&wtxn, &doc_id).unwrap();
	assert!(doc_length.is_some());
	assert!(doc_length.unwrap() > 0);

	// check that metadata was updated
	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap();
	assert!(metadata_bytes.is_some());

	let metadata: BM25Metadata = postcard::from_bytes(metadata_bytes.unwrap()).unwrap();
	assert_eq!(metadata.total_docs, 1);
	assert!(metadata.avgdl > 0.0);

	wtxn.commit().unwrap();
}

#[test]
fn test_insert_multiple_documents() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	let docs = vec![
		(1u128, "The quick brown fox"),
		(2u128, "jumps over the lazy dog"),
		(3u128, "machine learning algorithms"),
	];

	for (doc_id, doc) in &docs {
		let result = bm25.insert_doc(&mut wtxn, *doc_id, doc);
		assert!(result.is_ok());
	}

	// check metadata
	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap().unwrap();
	let metadata: BM25Metadata = postcard::from_bytes(metadata_bytes).unwrap();
	assert_eq!(metadata.total_docs, 3);

	wtxn.commit().unwrap();
}

#[test]
fn test_update_document() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	let doc_id = 1u128;

	// insert original document
	bm25.insert_doc(&mut wtxn, doc_id, "original content")
		.unwrap();

	// update document
	bm25.update_doc(&mut wtxn, doc_id, "updated content with more words")
		.unwrap();

	// check that document length was updated
	let doc_length = bm25.doc_lengths_db.get(&wtxn, &doc_id).unwrap().unwrap();
	assert!(doc_length > 2); // Should reflect the new document length

	wtxn.commit().unwrap();

	// search should find the updated content
	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "updated", 10, &arena).unwrap();
	assert_eq!(results.len(), 1);
	assert_eq!(results[0].0, doc_id);
}

#[test]
fn test_delete_document() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	let docs = vec![
		(1u128, "document one content"),
		(2u128, "document two content"),
		(3u128, "document three content"),
	];

	// insert documents
	for (doc_id, doc) in &docs {
		bm25.insert_doc(&mut wtxn, *doc_id, doc).unwrap();
	}

	// delete document 2
	bm25.delete_doc(&mut wtxn, 2u128).unwrap();

	// check that document length was removed
	let doc_length = bm25.doc_lengths_db.get(&wtxn, &2u128).unwrap();
	assert!(doc_length.is_none());

	// check that metadata was updated
	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap().unwrap();
	let metadata: BM25Metadata = postcard::from_bytes(metadata_bytes).unwrap();
	assert_eq!(metadata.total_docs, 2); // Should be reduced by 1

	wtxn.commit().unwrap();

	// search should not find the deleted document
	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "two", 10, &arena).unwrap();
	assert_eq!(results.len(), 0);
}

#[test]
fn test_delete_last_document_avgdl_becomes_zero() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	// Insert and then delete the only document
	bm25.insert_doc(&mut wtxn, 1u128, "only document").unwrap();
	bm25.delete_doc(&mut wtxn, 1u128).unwrap();

	// Check metadata after deleting last document
	let metadata_bytes = bm25.metadata_db.get(&wtxn, METADATA_KEY).unwrap().unwrap();
	let metadata: BM25Metadata = postcard::from_bytes(metadata_bytes).unwrap();

	assert_eq!(metadata.total_docs, 0);
	assert_eq!(metadata.avgdl, 0.0);

	wtxn.commit().unwrap();
}

#[test]
fn test_insert_document_with_repeated_terms() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	// Document with repeated terms
	bm25.insert_doc(&mut wtxn, 1u128, "test test test unique word word")
		.unwrap();

	// Document length should count all tokens (including duplicates)
	// "test", "test", "test", "unique", "word", "word" = 6 tokens
	let doc_length = bm25.doc_lengths_db.get(&wtxn, &1u128).unwrap().unwrap();
	assert_eq!(doc_length, 6);

	wtxn.commit().unwrap();

	// Search should still work
	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "test", 10, &arena).unwrap();
	assert_eq!(results.len(), 1);
}
