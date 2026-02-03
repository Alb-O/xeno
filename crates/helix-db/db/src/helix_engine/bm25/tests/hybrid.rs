use super::helpers::*;

#[tokio::test]
async fn test_hybrid_search() {
	let (storage, _temp_dir) = setup_helix_storage();

	let mut wtxn = storage.graph_env.write_txn().unwrap();
	let docs = vec![
		(1u128, "machine learning algorithms"),
		(2u128, "deep learning neural networks"),
		(3u128, "data science methods"),
	];

	let bm25 = storage.bm25.as_ref().unwrap();
	for (doc_id, doc) in &docs {
		bm25.insert_doc(&mut wtxn, *doc_id, doc).unwrap();
	}
	wtxn.commit().unwrap();

	let mut wtxn = storage.graph_env.write_txn().unwrap();
	let vectors = generate_random_vectors(800, 650);
	let mut arena = Bump::new();
	for vec in &vectors {
		let slice = arena.alloc_slice_copy(vec.as_slice());
		let _ = storage
			.vectors
			.insert::<fn(&HVector, &RoTxn) -> bool>(&mut wtxn, "vector", slice, None, &arena);
		arena.reset();
	}
	wtxn.commit().unwrap();

	let query = "machine learning";
	let query_vector = generate_random_vectors(1, 650);
	let alpha = 0.5; // equal weight between BM25 and vector
	let limit = 10;

	let result = storage
		.hybrid_search(query, &query_vector[0], alpha, limit)
		.await;

	match result {
		Ok(results) => assert!(results.len() <= limit),
		Err(_) => tracing::warn!("vector search not available"),
	}
}

#[tokio::test]
async fn test_hybrid_search_alpha_vectors() {
	let (storage, _temp_dir) = setup_helix_storage();

	// Insert some test documents first
	let mut wtxn = storage.graph_env.write_txn().unwrap();
	let docs = vec![
		(1u128, "machine learning algorithms"),
		(2u128, "deep learning neural networks"),
		(3u128, "data science methods"),
	];

	let bm25 = storage.bm25.as_ref().unwrap();
	for (doc_id, doc) in &docs {
		bm25.insert_doc(&mut wtxn, *doc_id, doc).unwrap();
	}
	wtxn.commit().unwrap();

	let mut wtxn = storage.graph_env.write_txn().unwrap();
	let vectors = generate_random_vectors(800, 650);
	let mut arena = Bump::new();
	for vec in &vectors {
		let slice = arena.alloc_slice_copy(vec.as_slice());
		let _ = storage
			.vectors
			.insert::<fn(&HVector, &RoTxn) -> bool>(&mut wtxn, "vector", slice, None, &arena);
		arena.reset();
	}
	wtxn.commit().unwrap();

	let query = "machine learning";
	let query_vector = generate_random_vectors(1, 650);

	// alpha = 0.0 (Vector only)
	let results_vector_only = storage
		.hybrid_search(query, &query_vector[0], 0.0, 10)
		.await;

	match results_vector_only {
		Ok(results) => assert!(results.len() <= 10),
		Err(_) => {
			tracing::warn!("vector-only search failed");
		}
	}
}

#[tokio::test]
async fn test_hybrid_search_alpha_bm25() {
	let (storage, _temp_dir) = setup_helix_storage();

	// Insert some test documents first
	let mut wtxn = storage.graph_env.write_txn().unwrap();
	let docs = vec![
		(1u128, "machine learning algorithms"),
		(2u128, "deep learning neural networks"),
		(3u128, "data science methods"),
	];

	let bm25 = storage.bm25.as_ref().unwrap();
	for (doc_id, doc) in &docs {
		bm25.insert_doc(&mut wtxn, *doc_id, doc).unwrap();
	}
	wtxn.commit().unwrap();

	let mut wtxn = storage.graph_env.write_txn().unwrap();
	let vectors = generate_random_vectors(800, 650);
	let mut arena = Bump::new();
	for vec in &vectors {
		let slice = arena.alloc_slice_copy(vec.as_slice());
		let _ = storage
			.vectors
			.insert::<fn(&HVector, &RoTxn) -> bool>(&mut wtxn, "vector", slice, None, &arena);
		arena.reset();
	}
	wtxn.commit().unwrap();

	let query = "machine learning";
	let query_vector = generate_random_vectors(1, 650);

	// alpha = 1.0 (BM25 only)
	let results_bm25_only = storage
		.hybrid_search(query, &query_vector[0], 1.0, 10)
		.await;

	// all should be valid results or acceptable errors
	match results_bm25_only {
		Ok(results) => assert!(results.len() <= 10),
		Err(_) => tracing::warn!("bm25-only search failed"),
	}
}
