use crate::Scoring;

const TRACE_DIAG: u8 = 0;
const TRACE_UP: u8 = 1;
const TRACE_LEFT: u8 = 2;

/// Reference scalar Smith-Waterman implementation with matched-count tracking.
///
/// Returns `(score, typos, score_matrix, exact)` under the full-needle contract:
/// score is the best alignment consuming the whole needle (max over last row),
/// and typos = `needle_len - matched_chars` counted on the same argmax alignment.
///
/// Matched-count tracking runs in lockstep with the scoring DP using the exact
/// same predecessor selection (score-based with diag > up > left tie-break).
pub fn smith_waterman(needle: &str, haystack: &str, scoring: &Scoring) -> (u16, u16, Vec<Vec<u16>>, bool) {
	let (score, typos, score_matrix, _, exact) = smith_waterman_internal(needle, haystack, scoring, false);
	(score, typos, score_matrix, exact)
}

/// Reference scalar Smith-Waterman including traceback indices for the full-needle path.
///
/// Returns `(score, typos, indices, exact)` where `indices` are matched haystack
/// character positions traced from the same argmax endpoint and predecessor
/// choices used for typo counting.
pub fn smith_waterman_with_indices(needle: &str, haystack: &str, scoring: &Scoring) -> (u16, u16, Vec<usize>, bool) {
	let (score, typos, _, indices, exact) = smith_waterman_internal(needle, haystack, scoring, true);
	(score, typos, indices.expect("indices requested"), exact)
}

fn smith_waterman_internal(needle: &str, haystack: &str, scoring: &Scoring, trace_indices: bool) -> (u16, u16, Vec<Vec<u16>>, Option<Vec<usize>>, bool) {
	let needle = needle.as_bytes();
	let haystack = haystack.as_bytes();
	let mut delimiter_table = [false; 256];
	for delimiter in scoring.delimiters.bytes() {
		delimiter_table[delimiter.to_ascii_lowercase() as usize] = true;
	}

	if needle.is_empty() {
		return (0, 0, vec![], trace_indices.then(Vec::new), haystack.is_empty());
	}
	if haystack.is_empty() {
		return (0, needle.len() as u16, vec![], trace_indices.then(Vec::new), false);
	}

	// State: score and matched-count matrices
	let mut score_matrix = vec![vec![0u16; haystack.len()]; needle.len()];
	let mut match_matrix = vec![vec![0u16; haystack.len()]; needle.len()];
	let mut trace_matrix = trace_indices.then(|| vec![vec![TRACE_DIAG; haystack.len()]; needle.len()]);

	for i in 0..needle.len() {
		let (prev_col_scores, curr_col_scores) = if i > 0 {
			let (prev, curr) = score_matrix.split_at_mut(i);
			(&prev[i - 1] as &Vec<u16>, &mut curr[0])
		} else {
			(&vec![0u16; haystack.len()], &mut score_matrix[0])
		};

		let (prev_col_matched, curr_col_matched) = if i > 0 {
			let (prev, curr) = match_matrix.split_at_mut(i);
			(&prev[i - 1] as &Vec<u16>, &mut curr[0])
		} else {
			(&vec![0u16; haystack.len()], &mut match_matrix[0])
		};

		let mut up_score: u16 = 0;
		let mut up_matched: u16 = 0;
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

			let haystack_char = haystack[j];
			let haystack_is_uppercase = haystack_char.is_ascii_uppercase();
			let haystack_is_lowercase = haystack_char.is_ascii_lowercase();
			let haystack_char = haystack_char.to_ascii_lowercase();

			let haystack_is_delimiter = delimiter_table[haystack_char as usize];
			let matched_casing_mask = needle_is_uppercase == haystack_is_uppercase;

			let match_score = if is_prefix {
				scoring.match_score + scoring.prefix_bonus
			} else if is_offset_prefix {
				scoring.match_score + scoring.offset_prefix_bonus
			} else {
				scoring.match_score
			};

			// Diagonal predecessors
			let diag_prev = if is_prefix { 0 } else { prev_col_scores[j - 1] };
			let diag_matched_prev = if is_prefix { 0 } else { prev_col_matched[j - 1] };
			let is_match = needle_char == haystack_char;
			let diag_score = if is_match {
				diag_prev
					+ match_score + if prev_haystack_is_delimiter && delimiter_bonus_enabled && !haystack_is_delimiter {
					scoring.delimiter_bonus
				} else {
					0
				} + if !is_prefix && haystack_is_uppercase && prev_haystack_is_lowercase {
					scoring.capitalization_bonus
				} else {
					0
				} + if matched_casing_mask { scoring.matching_case_bonus } else { 0 }
			} else {
				diag_prev.saturating_sub(scoring.mismatch_penalty)
			};
			let diag_matched = if is_match { diag_matched_prev + 1 } else { diag_matched_prev };

			// Up predecessor (skip haystack char)
			let up_gap_penalty = if up_gap_penalty_mask {
				scoring.gap_open_penalty
			} else {
				scoring.gap_extend_penalty
			};
			let up_score_val = up_score.saturating_sub(up_gap_penalty);
			let up_matched_val = up_matched;

			// Left predecessor (skip needle char)
			let left_prev = prev_col_scores[j];
			let left_matched_prev = prev_col_matched[j];
			let left_gap_penalty = if left_gap_penalty_mask {
				scoring.gap_open_penalty
			} else {
				scoring.gap_extend_penalty
			};
			let left_score = left_prev.saturating_sub(left_gap_penalty);
			let left_matched_val = left_matched_prev;

			// Winner selection: same as original DP (score-based, diag wins ties)
			let max_score = diag_score.max(up_score_val).max(left_score);
			let diag_wins = max_score == diag_score;
			let up_wins = !diag_wins && max_score == up_score_val;
			if let Some(trace_matrix) = trace_matrix.as_mut() {
				trace_matrix[i][j] = if diag_wins {
					TRACE_DIAG
				} else if up_wins {
					TRACE_UP
				} else {
					TRACE_LEFT
				};
			}

			let max_matched = if diag_wins {
				diag_matched
			} else if up_wins {
				up_matched_val
			} else {
				left_matched_val
			};

			// Gap penalty mask update (same logic as original)
			let diag_mask = max_score == diag_score;
			up_gap_penalty_mask = max_score != up_score_val || diag_mask;
			left_gap_penalty_mask = max_score != left_score || diag_mask;

			prev_haystack_is_lowercase = haystack_is_lowercase;
			prev_haystack_is_delimiter = haystack_is_delimiter;
			delimiter_bonus_enabled |= !prev_haystack_is_delimiter;

			up_score = max_score;
			up_matched = max_matched;
			curr_col_scores[j] = max_score;
			curr_col_matched[j] = max_matched;
		}
	}

	// Full-needle contract: score and matched from last needle row argmax.
	let last_row_scores = score_matrix.last().unwrap();
	let last_row_matched = match_matrix.last().unwrap();

	let mut best_score = last_row_scores[0];
	let mut best_matched = last_row_matched[0];
	let mut best_row = 0usize;
	for j in 1..haystack.len() {
		if last_row_scores[j] > best_score {
			best_score = last_row_scores[j];
			best_matched = last_row_matched[j];
			best_row = j;
		}
	}

	let exact = haystack == needle;
	if exact {
		best_score += scoring.exact_match_bonus;
	}

	let typos = (needle.len() as u16).saturating_sub(best_matched);

	let indices = trace_matrix.map(|trace_matrix| traceback_indices(needle, haystack, &trace_matrix, best_row));

	(best_score, typos, score_matrix, indices, exact)
}

fn traceback_indices(needle: &[u8], haystack: &[u8], trace_matrix: &[Vec<u8>], start_row: usize) -> Vec<usize> {
	let mut needle_idx = needle.len() - 1;
	let mut haystack_idx = start_row;
	let mut indices = Vec::new();

	loop {
		match trace_matrix[needle_idx][haystack_idx] {
			TRACE_DIAG => {
				if needle[needle_idx].eq_ignore_ascii_case(&haystack[haystack_idx]) {
					indices.push(haystack_idx);
				}
				if needle_idx == 0 || haystack_idx == 0 {
					break;
				}
				needle_idx -= 1;
				haystack_idx -= 1;
			}
			TRACE_UP => {
				if haystack_idx == 0 {
					break;
				}
				haystack_idx -= 1;
			}
			TRACE_LEFT => {
				if needle_idx == 0 {
					break;
				}
				needle_idx -= 1;
			}
			_ => unreachable!("invalid traceback direction"),
		}
	}

	indices.reverse();
	indices
}

#[cfg(test)]
mod tests;
