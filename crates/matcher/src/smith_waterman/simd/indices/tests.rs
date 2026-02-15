use super::*;
use crate::Scoring;
use crate::smith_waterman::simd::smith_waterman;

fn get_indices(needle: &str, haystack: &str) -> Vec<usize> {
	let haystacks = [haystack; 1];
	let (_, score_matrices, _) = smith_waterman::<16, 1>(needle, &haystacks, None, &Scoring::default());
	let indices = char_indices_from_score_matrix(&score_matrices);
	indices[0].clone()
}

#[test]
fn test_leaking() {
	let needle = "t";
	let haystacks = [
		"true",
		"toDate",
		"toString",
		"transpose",
		"testing",
		"to",
		"toRgba",
		"toolbar",
		"true",
		"toDate",
		"toString",
		"transpose",
		"testing",
		"to",
		"toRgba",
		"toolbar",
	];

	let (_, score_matrices, _) = smith_waterman::<16, 16>(needle, &haystacks, None, &Scoring::default());
	let indices = char_indices_from_score_matrix(&score_matrices);
	for indices in indices.into_iter() {
		assert_eq!(indices, [0])
	}
}

#[test]
fn test_basic_indices() {
	assert_eq!(get_indices("b", "abc"), vec![1]);
	assert_eq!(get_indices("c", "abc"), vec![2]);
}

#[test]
fn test_prefix_indices() {
	assert_eq!(get_indices("a", "abc"), vec![0]);
	assert_eq!(get_indices("a", "aabc"), vec![0]);
	assert_eq!(get_indices("a", "babc"), vec![1]);
}

#[test]
fn test_exact_match_indices() {
	assert_eq!(get_indices("a", "a"), vec![0]);
	assert_eq!(get_indices("abc", "abc"), vec![0, 1, 2]);
	assert_eq!(get_indices("ab", "abc"), vec![0, 1]);
}

#[test]
fn test_delimiter_indices() {
	assert_eq!(get_indices("b", "a-b"), vec![2]);
	assert_eq!(get_indices("a", "a-b-c"), vec![0]);
	assert_eq!(get_indices("b", "a--b"), vec![3]);
	assert_eq!(get_indices("c", "a--bc"), vec![4]);
}

#[test]
fn test_affine_gap_indices() {
	assert_eq!(get_indices("test", "Uterst"), vec![1, 2, 4, 5]);
	assert_eq!(get_indices("test", "Uterrst"), vec![1, 2, 5, 6]);
	assert_eq!(get_indices("test", "Uterrs t"), vec![1, 2, 5, 7]);
}

#[test]
fn test_capital_indices() {
	assert_eq!(get_indices("a", "A"), vec![0]);
	assert_eq!(get_indices("A", "Aa"), vec![0]);
	assert_eq!(get_indices("D", "forDist"), vec![3]);
}

#[test]
fn test_typo_indices() {
	assert_eq!(get_indices("b", "a"), Vec::<usize>::new());
	assert_eq!(get_indices("reba", "repack"), vec![0, 1, 3]);
	assert_eq!(get_indices("bbb", "abc"), vec![1]);
}
