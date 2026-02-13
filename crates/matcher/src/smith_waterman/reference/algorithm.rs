use crate::Scoring;

pub fn smith_waterman(needle: &str, haystack: &str, scoring: &Scoring) -> (u16, Vec<Vec<u16>>, bool) {
	let needle = needle.as_bytes();
	let haystack = haystack.as_bytes();
	let mut delimiter_table = [false; 256];
	for delimiter in scoring.delimiters.bytes() {
		delimiter_table[delimiter.to_ascii_lowercase() as usize] = true;
	}

	// State
	let mut score_matrix = vec![vec![0; haystack.len()]; needle.len()];
	let mut all_time_max_score = 0;

	for i in 0..needle.len() {
		let (prev_col_scores, curr_col_scores) = if i > 0 {
			let (prev_col_scores_slice, curr_col_scores_slice) = score_matrix.split_at_mut(i);
			(&prev_col_scores_slice[i - 1], &mut curr_col_scores_slice[0])
		} else {
			(&vec![0; haystack.len()], &mut score_matrix[i])
		};

		let mut up_score_simd: u16 = 0;
		let mut up_gap_penalty_mask = true;

		let needle_char = needle[i];
		let needle_is_uppercase = needle_char.is_ascii_uppercase();
		let needle_char = needle_char.to_ascii_lowercase();

		let mut left_gap_penalty_mask = true;
		let mut delimiter_bonus_enabled = false;
		let mut prev_haystack_is_delimiter = false;
		let mut prev_haystack_is_lowercase = false;

		for j in 0..haystack.len() {
			let is_prefix = j == 0;
			let is_offset_prefix = j == 1 && prev_col_scores[0] == 0 && !haystack[0].is_ascii_alphabetic();

			// Load chunk and remove casing
			let haystack_char = haystack[j];
			let haystack_is_uppercase = haystack_char.is_ascii_uppercase();
			let haystack_is_lowercase = haystack_char.is_ascii_lowercase();
			let haystack_char = haystack_char.to_ascii_lowercase();

			let haystack_is_delimiter = delimiter_table[haystack_char as usize];
			let matched_casing_mask = needle_is_uppercase == haystack_is_uppercase;

			// Give a bonus for prefix matches
			let match_score = if is_prefix {
				scoring.match_score + scoring.prefix_bonus
			} else if is_offset_prefix {
				scoring.match_score + scoring.offset_prefix_bonus
			} else {
				scoring.match_score
			};

			// Calculate diagonal (match/mismatch) scores
			let diag = if is_prefix { 0 } else { prev_col_scores[j - 1] };
			let is_match = needle_char == haystack_char;
			let diag_score = if is_match {
				diag + match_score
					+ if prev_haystack_is_delimiter && delimiter_bonus_enabled && !haystack_is_delimiter {
						scoring.delimiter_bonus
					} else {
						0
					}
                    // ignore capitalization on the prefix
					+ if !is_prefix && haystack_is_uppercase && prev_haystack_is_lowercase {
						scoring.capitalization_bonus
					} else {
						0
					}
					+ if matched_casing_mask {
						scoring.matching_case_bonus
					} else {
						0
					}
			} else {
				diag.saturating_sub(scoring.mismatch_penalty)
			};

			// Load and calculate up scores (skipping char in haystack)
			let up_gap_penalty = if up_gap_penalty_mask {
				scoring.gap_open_penalty
			} else {
				scoring.gap_extend_penalty
			};
			let up_score = up_score_simd.saturating_sub(up_gap_penalty);

			// Load and calculate left scores (skipping char in needle)
			let left = prev_col_scores[j];
			let left_gap_penalty = if left_gap_penalty_mask {
				scoring.gap_open_penalty
			} else {
				scoring.gap_extend_penalty
			};
			let left_score = left.saturating_sub(left_gap_penalty);

			// Calculate maximum scores
			let max_score = diag_score.max(up_score).max(left_score);

			// Update gap penalty mask
			let diag_mask = max_score == diag_score;
			up_gap_penalty_mask = max_score != up_score || diag_mask;
			left_gap_penalty_mask = max_score != left_score || diag_mask;

			// Update haystack char masks
			prev_haystack_is_lowercase = haystack_is_lowercase;
			prev_haystack_is_delimiter = haystack_is_delimiter;
			// Only enable delimiter bonus if we've seen a non-delimiter char
			delimiter_bonus_enabled |= !prev_haystack_is_delimiter;

			// Store the scores for the next iterations
			up_score_simd = max_score;
			curr_col_scores[j] = max_score;

			// Store the maximum score across all runs
			all_time_max_score = all_time_max_score.max(max_score);
		}
	}

	let mut max_score = all_time_max_score;
	let exact = haystack == needle;
	if exact {
		max_score += scoring.exact_match_bonus;
	}

	(max_score, score_matrix, exact)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Scoring;
	use crate::r#const::*;
	use crate::smith_waterman::reference::typos_from_score_matrix;
	use crate::smith_waterman::simd::{smith_waterman as smith_waterman_simd, smith_waterman_scores_typos};

	const CHAR_SCORE: u16 = MATCH_SCORE + MATCHING_CASE_BONUS;

	fn get_score(needle: &str, haystack: &str) -> u16 {
		get_score_with_scoring(needle, haystack, &Scoring::default())
	}

	fn get_score_with_scoring(needle: &str, haystack: &str, scoring: &Scoring) -> u16 {
		let ref_score = smith_waterman(needle, haystack, scoring).0;
		let simd_score = smith_waterman_simd::<16, 1>(needle, &[haystack], None, scoring).0[0];

		assert_eq!(ref_score, simd_score, "Reference and SIMD scores don't match");

		ref_score
	}

	fn ref_score_typos_exact(needle: &str, haystack: &str, scoring: &Scoring) -> (u16, u16, bool) {
		let (score, score_matrix, exact) = smith_waterman(needle, haystack, scoring);
		let score_matrix_ref = score_matrix.iter().map(|row| row.as_slice()).collect::<Vec<_>>();
		let typos = typos_from_score_matrix(&score_matrix_ref);
		(score, typos, exact)
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
		assert_eq!(get_score("ab", "abc"), 2 * CHAR_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("abc", "ab"), 2 * CHAR_SCORE + PREFIX_BONUS);
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
	fn test_colon_delimiter_bonus_applies() {
		assert_eq!(get_score("b", "a:b"), CHAR_SCORE + DELIMITER_BONUS);
	}

	#[test]
	fn test_custom_delimiter_set_changes_bonus_behavior() {
		let mut scoring = Scoring::default();
		scoring.delimiters = "@".to_string();

		assert_eq!(get_score_with_scoring("b", "a@b", &scoring), CHAR_SCORE + DELIMITER_BONUS);
		assert_eq!(get_score_with_scoring("b", "a_b", &scoring), CHAR_SCORE);
	}

	#[test]
	fn simd_typo_counts_and_gating_match_reference() {
		fn assert_case(needle: &str, haystack: &str, scoring: &Scoring) {
			let (ref_score, ref_typos, ref_exact) = ref_score_typos_exact(needle, haystack, scoring);

			for max_typos in 0..=3 {
				let (simd_scores, simd_typos, simd_exacts) = smith_waterman_scores_typos::<16, 1>(needle, &[haystack], max_typos, scoring);
				let simd_score = simd_scores[0];
				let simd_typo_count = simd_typos[0];

				let context = format!(
					"needle={needle:?}, haystack={haystack:?}, max_typos={max_typos}, ref_typos={ref_typos}, simd_typos={simd_typo_count}, ref_score={ref_score}, simd_score={simd_score}"
				);

				if ref_typos <= max_typos {
					assert_eq!(simd_typo_count, ref_typos, "{context}");
					assert_eq!(simd_score, ref_score, "{context}");
				} else {
					assert!(simd_typo_count > max_typos, "{context}");
				}

				assert_eq!(simd_exacts[0], ref_exact, "{context}");
			}
		}

		let scoring = Scoring::default();
		let fixed_cases = vec![
			("deadbeef".to_string(), "deadbeef".to_string()),
			("deadbeef".to_string(), "deadbee".to_string()),
			("abcdef".to_string(), "abc".to_string()),
			("_-_".to_string(), "---___".to_string()),
			("AbC".to_string(), "a_bc".to_string()),
		];

		for (needle, haystack) in fixed_cases {
			assert_case(&needle, &haystack, &scoring);
		}

		let mut rng = XorShift64::new(0x9173_D5B2_4C8E_11A7);
		let alphabet = b"abcdeABCDE_-/";
		for _ in 0..1000 {
			let needle_len = rng.next_usize(12) + 1;
			let haystack_len = rng.next_usize(16) + 1;
			let needle = String::from_utf8(gen_ascii_bytes(&mut rng, needle_len, alphabet)).expect("needle is valid ASCII");
			let haystack = String::from_utf8(gen_ascii_bytes(&mut rng, haystack_len, alphabet)).expect("haystack is valid ASCII");
			assert_case(&needle, &haystack, &scoring);
		}
	}
}
