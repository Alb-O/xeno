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
mod tests;
