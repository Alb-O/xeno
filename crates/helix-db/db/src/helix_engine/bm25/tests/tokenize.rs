use super::helpers::*;

#[test]
fn test_tokenize_with_filter() {
	let (bm25, _temp_dir) = setup_bm25_config();

	let text = "The quick brown fox jumps over the lazy dog! It was amazing.";
	let tokens = bm25.tokenize::<true>(text);

	// should filter out words with length <= 2 and normalize to lowercase
	let expected = [
		"the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "was", "amazing",
	];
	assert_eq!(tokens.len(), expected.len());

	for (i, token) in tokens.iter().enumerate() {
		assert_eq!(token, expected[i]);
	}
}

#[test]
fn test_tokenize_without_filter() {
	let (bm25, _temp_dir) = setup_bm25_config();

	let text = "A B CD efg!";
	let tokens = bm25.tokenize::<false>(text);

	// should not filter out short words
	let expected = ["a", "b", "cd", "efg"];
	assert_eq!(tokens.len(), expected.len());

	for (i, token) in tokens.iter().enumerate() {
		assert_eq!(token, expected[i]);
	}
}

#[test]
fn test_tokenize_edge_cases_punctuation_only() {
	let (bm25, _temp_dir) = setup_bm25_config();

	let tokens = bm25.tokenize::<true>("!@#$%^&*()");
	assert_eq!(tokens.len(), 0);
}

#[test]
fn test_tokenize_empty_string() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let tokens = bm25.tokenize::<true>("");
	assert_eq!(tokens.len(), 0);
}

#[test]
fn test_tokenize_whitespace_only() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let tokens = bm25.tokenize::<true>("   \t\n   ");
	assert_eq!(tokens.len(), 0);
}

#[test]
fn test_tokenize_with_numbers() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let tokens = bm25.tokenize::<true>("test123 456abc");
	assert!(tokens.contains(&"test123".to_string()));
	assert!(tokens.contains(&"456abc".to_string()));
}

#[test]
fn test_tokenize_unicode() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let tokens = bm25.tokenize::<false>("日本語 русский français");
	// Should handle unicode alphanumeric characters
	assert!(!tokens.is_empty());
}

#[test]
fn test_tokenize_mixed_case() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let tokens = bm25.tokenize::<false>("HELLO hello HeLLo");
	// All should be lowercase
	for token in &tokens {
		assert_eq!(token, "hello");
	}
	assert_eq!(tokens.len(), 3);
}
