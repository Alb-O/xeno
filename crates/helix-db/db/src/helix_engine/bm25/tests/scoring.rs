use super::helpers::*;

#[test]
fn test_bm25_score_calculation() {
	let (bm25, _temp_dir) = setup_bm25_config();

	let score = bm25.calculate_bm25_score(
		2,   // term frequency
		10,  // doc length
		3,   // document frequency
		100, // total docs
		8.0, // average doc length
	);

	tracing::debug!(score, "bm25 score calculation");

	// Score should be finite and reasonable
	assert!(score.is_finite());
	assert!(score != 0.0);
}

#[test]
fn test_bm25_score_properties() {
	let (bm25, _temp_dir) = setup_bm25_config();

	// test that higher term frequency yields higher score
	let score1 = bm25.calculate_bm25_score(1, 10, 5, 100, 10.0);
	let score2 = bm25.calculate_bm25_score(3, 10, 5, 100, 10.0);
	assert!(score2 > score1);

	// test that rare terms (lower df) yield higher scores
	let score_rare = bm25.calculate_bm25_score(1, 10, 2, 100, 10.0);
	let score_common = bm25.calculate_bm25_score(1, 10, 50, 100, 10.0);
	assert!(score_rare > score_common);
}

#[test]
fn test_calculate_bm25_score_edge_case_zero_avgdl() {
	let (bm25, _temp_dir) = setup_bm25_config();
	// When avgdl is 0, should use doc_len as fallback
	let score = bm25.calculate_bm25_score(1, 10, 5, 100, 0.0);
	assert!(score.is_finite());
}

#[test]
fn test_calculate_bm25_score_edge_case_zero_total_docs() {
	let (bm25, _temp_dir) = setup_bm25_config();
	// Edge case: 0 total docs (uses max(1))
	let score = bm25.calculate_bm25_score(1, 10, 5, 0, 10.0);
	assert!(score.is_finite());
}

#[test]
fn test_calculate_bm25_score_edge_case_zero_df() {
	let (bm25, _temp_dir) = setup_bm25_config();
	// Edge case: 0 document frequency (uses max(1))
	let score = bm25.calculate_bm25_score(1, 10, 0, 100, 10.0);
	assert!(score.is_finite());
}

#[test]
fn test_calculate_bm25_score_high_df_low_idf() {
	let (bm25, _temp_dir) = setup_bm25_config();
	// When df is very high relative to total_docs, IDF can be negative
	let score = bm25.calculate_bm25_score(1, 10, 95, 100, 10.0);
	// Score should still be finite (may be negative with high df)
	assert!(score.is_finite());
}

#[test]
fn test_calculate_bm25_score_very_short_document() {
	let (bm25, _temp_dir) = setup_bm25_config();
	// Very short document with doc_len = 1
	let score = bm25.calculate_bm25_score(1, 1, 5, 100, 10.0);
	assert!(score.is_finite());
	assert!(score > 0.0);
}

#[test]
fn test_calculate_bm25_score_very_long_document() {
	let (bm25, _temp_dir) = setup_bm25_config();
	// Very long document
	let score = bm25.calculate_bm25_score(5, 10000, 5, 100, 100.0);
	assert!(score.is_finite());
}
