use super::*;
use crate::r#const::*;

const CHAR_SCORE: u16 = MATCH_SCORE + MATCHING_CASE_BONUS;

fn get_score(needle: &str, haystack: &str) -> u16 {
	smith_waterman::<16, 1>(needle, &[haystack], None, &Scoring::default()).0[0]
}

#[test]
fn test_score_basic() {
	assert_eq!(get_score("b", "abc"), CHAR_SCORE);
	assert_eq!(get_score("c", "abc"), CHAR_SCORE);
}

#[test]
fn test_score_prefix() {
	assert_eq!(get_score("a", "abc"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("a", "aabc"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("a", "babc"), CHAR_SCORE);
}

#[test]
fn test_score_offset_prefix() {
	// Give prefix bonus on second char if the first char isn't a letter
	assert_eq!(get_score("a", "-a"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
	assert_eq!(get_score("-a", "-ab"), 2 * CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("a", "'a"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
	assert_eq!(get_score("a", "Ba"), CHAR_SCORE);
}

#[test]
fn test_score_exact_match() {
	assert_eq!(get_score("a", "a"), CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
	assert_eq!(get_score("abc", "abc"), 3 * CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
}

#[test]
fn test_score_delimiter() {
	assert_eq!(get_score("-", "a--bc"), CHAR_SCORE);
	assert_eq!(get_score("b", "a-b"), CHAR_SCORE + DELIMITER_BONUS);
	assert_eq!(get_score("a", "a-b-c"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("b", "a--b"), CHAR_SCORE + DELIMITER_BONUS);
	assert_eq!(get_score("c", "a--bc"), CHAR_SCORE);
	assert_eq!(get_score("a", "-a--bc"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
}

#[test]
fn test_score_no_delimiter_for_delimiter_chars() {
	assert_eq!(get_score("-", "a-bc"), CHAR_SCORE);
	assert_eq!(get_score("-", "a--bc"), CHAR_SCORE);
	assert!(get_score("a_b", "a_bb") > get_score("a_b", "a__b"));
}

#[test]
fn test_score_affine_gap() {
	assert_eq!(get_score("test", "Uterst"), CHAR_SCORE * 4 - GAP_OPEN_PENALTY);
	assert_eq!(get_score("test", "Uterrst"), CHAR_SCORE * 4 - GAP_OPEN_PENALTY - GAP_EXTEND_PENALTY);
}

#[test]
fn test_score_capital_bonus() {
	assert_eq!(get_score("a", "A"), MATCH_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("A", "Aa"), CHAR_SCORE + PREFIX_BONUS);
	assert_eq!(get_score("D", "forDist"), CHAR_SCORE + CAPITALIZATION_BONUS);
	assert_eq!(get_score("D", "foRDist"), CHAR_SCORE);
	assert_eq!(get_score("D", "FOR_DIST"), CHAR_SCORE + DELIMITER_BONUS);
}

#[test]
fn test_score_prefix_beats_delimiter() {
	assert!(get_score("swap", "swap(test)") > get_score("swap", "iter_swap(test)"));
	assert!(get_score("_", "_private_member") > get_score("_", "public_member"));
}

#[test]
fn test_score_prefix_beats_capitalization() {
	assert!(get_score("H", "HELLO") > get_score("H", "fooHello"));
}

#[test]
fn test_score_continuous_beats_delimiter() {
	assert!(get_score("foo", "fooo") > get_score("foo", "f_o_o_o"));
}

#[test]
fn test_score_continuous_beats_capitalization() {
	assert!(get_score("fo", "foo") > get_score("fo", "faOo"));
}

#[test]
fn four_way_score_comparison() {
	use crate::smith_waterman::reference::smith_waterman as ref_sw;

	let scoring = Scoring::default();
	let cases = vec![
		("Dd/aAd", "da--aD-ca/c-"),
		("deadbeef", "deadbeef"),
		("abc", "ab"),
		("ab", "abc"),
		("solf", "self::"),
	];

	for (needle, haystack) in &cases {
		let (ref_score, _, ref_exact) = ref_sw(needle, haystack, &scoring);
		let (scores_only, exact_only) = smith_waterman_scores::<16, 1>(needle, &[haystack], &scoring);
		let (matrix_scores, _, matrix_exact) = smith_waterman::<16, 1>(needle, &[haystack], None, &scoring);
		let (typo_scores, typo_counts, typo_exact) = smith_waterman_scores_typos::<16, 1>(needle, &[haystack], 3, &scoring);

		eprintln!("--- needle={needle:?} haystack={haystack:?} ---");
		eprintln!("  reference:    score={ref_score}, exact={ref_exact}");
		eprintln!("  scores_only:  score={}, exact={}", scores_only[0], exact_only[0]);
		eprintln!("  matrix(None): score={}, exact={}", matrix_scores[0], matrix_exact[0]);
		eprintln!("  scores_typos: score={}, typos={}, exact={}", typo_scores[0], typo_counts[0], typo_exact[0]);

		assert_eq!(ref_score, scores_only[0], "ref vs scores_only mismatch for {needle:?}/{haystack:?}");
		assert_eq!(ref_score, matrix_scores[0], "ref vs matrix mismatch for {needle:?}/{haystack:?}");
		assert_eq!(ref_score, typo_scores[0], "ref vs typo_scores mismatch for {needle:?}/{haystack:?}");
	}
}

#[test]
fn debug_traceback_divergence() {
	use crate::smith_waterman::reference::{smith_waterman as ref_sw, typos_from_score_matrix as ref_typos};

	let scoring = Scoring::default();
	let needle = "eb--E";
	let haystack = "eaADcAb";

	// Get reference matrix and compute typos both ways
	let (_, ref_matrix, _) = ref_sw(needle, haystack, &scoring);
	let ref_matrix_slices: Vec<&[u16]> = ref_matrix.iter().map(|v| v.as_slice()).collect();
	let ref_typo_count = ref_typos(&ref_matrix_slices);

	// Get SIMD matrix and compute typos
	let (_, simd_matrix, _) = smith_waterman::<16, 1>(needle, &[haystack], None, &scoring);
	let simd_typo_count = super::super::typos_from_score_matrix::<16, 1>(&simd_matrix, 100)[0];

	// Also compute reference typos on the SIMD matrix (extracted to scalar)
	let simd_as_scalar: Vec<Vec<u16>> = simd_matrix.iter().map(|col| col.iter().map(|s| s[0]).collect()).collect();
	let simd_scalar_slices: Vec<&[u16]> = simd_as_scalar.iter().map(|v| v.as_slice()).collect();
	let ref_typos_on_simd_matrix = ref_typos(&simd_scalar_slices);

	eprintln!("needle={needle:?} haystack={haystack:?}");
	eprintln!("ref_typos (ref matrix):  {ref_typo_count}");
	eprintln!("simd_typos (simd matrix): {simd_typo_count}");
	eprintln!("ref_typos (simd matrix): {ref_typos_on_simd_matrix}");

	// Print both matrices
	eprintln!("ref matrix:");
	for i in 0..needle.len() {
		let row: Vec<u16> = (0..haystack.len()).map(|j| ref_matrix[i][j]).collect();
		eprintln!("  [{i}] {:?}", row);
	}
	eprintln!("simd matrix (lane 0, full W=16):");
	for i in 0..needle.len() {
		let row: Vec<u16> = (0..16).map(|j| simd_matrix[i][j][0]).collect();
		eprintln!("  [{i}] {:?}", row);
	}
}

#[test]
fn score_typo_contract_parity() {
	use crate::smith_waterman::reference::smith_waterman as ref_sw;

	let scoring = Scoring::default();
	let mut rng = XorShift64::new(0xA3B7_C4D2_E1F0_9856);
	let alphabet = b"abcdeABCDE_-/";

	for _ in 0..500 {
		let needle_len = 1 + rng.next_usize(12);
		let haystack_len = 1 + rng.next_usize(16);
		let needle = String::from_utf8(gen_ascii_bytes(&mut rng, needle_len, alphabet)).unwrap();
		let haystack = String::from_utf8(gen_ascii_bytes(&mut rng, haystack_len, alphabet)).unwrap();

		let (ref_score, _, ref_exact) = ref_sw(&needle, &haystack, &scoring);
		let (scores_only, exact_only) = smith_waterman_scores::<16, 1>(&needle, &[haystack.as_str()], &scoring);

		assert_eq!(
			ref_score, scores_only[0],
			"ref vs scores_only: needle={needle:?} haystack={haystack:?}"
		);
		assert_eq!(ref_exact, exact_only[0]);

		for max_typos in [0u16, 1, 3] {
			let (typo_scores, typo_counts, typo_exacts) =
				smith_waterman_scores_typos::<16, 1>(&needle, &[haystack.as_str()], max_typos, &scoring);

			// scores_typos must produce the same score as score-only (independent of max_typos)
			assert_eq!(
				scores_only[0], typo_scores[0],
				"score_only vs score_typos: needle={needle:?} haystack={haystack:?} max_typos={max_typos}"
			);
			assert_eq!(exact_only[0], typo_exacts[0]);

			// internal consistency: scores_typos uses same matrix+traceback
			let (matrix_scores, matrix, matrix_exacts) =
				smith_waterman::<16, 1>(&needle, &[haystack.as_str()], None, &scoring);
			let matrix_typos = super::super::typos_from_score_matrix::<16, 1>(&matrix, max_typos);

			assert_eq!(typo_scores[0], matrix_scores[0]);
			assert_eq!(typo_counts[0], matrix_typos[0]);
			assert_eq!(typo_exacts[0], matrix_exacts[0]);
		}
	}
}

#[derive(Clone, Copy)]
struct XorShift64 {
	state: u64,
}

impl XorShift64 {
	fn new(seed: u64) -> Self {
		Self { state: seed.max(1) }
	}

	fn next_u64(&mut self) -> u64 {
		let mut x = self.state;
		x ^= x >> 12;
		x ^= x << 25;
		x ^= x >> 27;
		self.state = x;
		x.wrapping_mul(0x2545_F491_4F6C_DD1D)
	}

	fn next_usize(&mut self, upper_bound: usize) -> usize {
		if upper_bound <= 1 {
			return 0;
		}
		(self.next_u64() as usize) % upper_bound
	}
}

fn gen_ascii_bytes(rng: &mut XorShift64, len: usize, alphabet: &[u8]) -> Vec<u8> {
	let mut out = Vec::with_capacity(len);
	for _ in 0..len {
		out.push(alphabet[rng.next_usize(alphabet.len())]);
	}
	out
}

#[test]
fn scores_only_matches_matrix_scores() {
	let scoring = Scoring::default();
	let haystacks = ["alpha", "a-b", "forDist", "exact"];
	let needle = "a";

	let (scores_only, exact_only) = smith_waterman_scores::<16, 4>(needle, &haystacks, &scoring);
	let (scores_matrix, _, exact_matrix) = smith_waterman::<16, 4>(needle, &haystacks, None, &scoring);

	assert_eq!(scores_only, scores_matrix);
	assert_eq!(exact_only, exact_matrix);

	let needle_exact = "exact";
	let (scores_only_exact, exact_only_exact) = smith_waterman_scores::<16, 4>(needle_exact, &haystacks, &scoring);
	let (scores_matrix_exact, _, exact_matrix_exact) = smith_waterman::<16, 4>(needle_exact, &haystacks, None, &scoring);

	assert_eq!(scores_only_exact, scores_matrix_exact);
	assert_eq!(exact_only_exact, exact_matrix_exact);
}
