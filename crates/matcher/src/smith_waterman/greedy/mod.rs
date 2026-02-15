//! Greedy fallback fuzzy matching algorithm, which doesn't use Smith Waterman
//! to find the optimal alignment. Runs in linear time and used for when the Smith Waterman matrix
//! would balloon in size (due to being N * M)

use crate::Scoring;

#[inline]
fn delimiter_table(scoring: &Scoring) -> [bool; 256] {
	let mut table = [false; 256];
	for delimiter in scoring.delimiters.bytes() {
		table[delimiter.to_ascii_lowercase() as usize] = true;
	}
	table
}

pub fn match_greedy<S1: AsRef<str>, S2: AsRef<str>>(needle: S1, haystack: S2, scoring: &Scoring) -> (u16, Vec<usize>, bool) {
	let needle = needle.as_ref().as_bytes();
	let haystack = haystack.as_ref().as_bytes();
	if needle.is_empty() || haystack.is_empty() {
		return (0, vec![], haystack == needle);
	}
	let delimiter_table = delimiter_table(scoring);

	let mut score = 0;
	let mut indices = vec![];
	let mut haystack_idx = 0;

	let mut delimiter_bonus_enabled = false;
	let mut previous_haystack_is_lower = false;
	let mut previous_haystack_is_delimiter = false;
	'outer: for needle_idx in 0..needle.len() {
		let needle_char = needle[needle_idx];
		let needle_is_upper = (65..=90).contains(&needle_char);
		let needle_is_lower = (97..=122).contains(&needle_char);

		let needle_lower_char = if needle_is_upper { needle_char + 32 } else { needle_char };
		let needle_upper_char = if needle_is_lower { needle_char - 32 } else { needle_char };

		let haystack_start_idx = haystack_idx;
		let remaining_needle = needle.len() - needle_idx;
		while haystack_idx < haystack.len() && (haystack.len() - haystack_idx) >= remaining_needle {
			let haystack_char = haystack[haystack_idx];
			let haystack_is_delimiter = delimiter_table[haystack_char.to_ascii_lowercase() as usize];
			let haystack_is_upper = (65..=90).contains(&haystack_char);
			let haystack_is_lower = (97..=122).contains(&haystack_char);

			// Only enable delimiter bonus if we've seen a non-delimiter char
			if !haystack_is_delimiter {
				delimiter_bonus_enabled = true;
			}

			if needle_lower_char != haystack_char && needle_upper_char != haystack_char {
				previous_haystack_is_delimiter = delimiter_bonus_enabled && haystack_is_delimiter;
				previous_haystack_is_lower = haystack_is_lower;
				haystack_idx += 1;
				continue;
			}

			// found a match, add the scores and continue the outer loop
			score += scoring.match_score;

			// gap penalty
			if haystack_idx != haystack_start_idx && needle_idx != 0 {
				score =
					score.saturating_sub(scoring.gap_open_penalty + scoring.gap_extend_penalty * (haystack_idx - haystack_start_idx).saturating_sub(1) as u16);
			}

			// bonuses (see constant documentation for details)
			if needle_char == haystack_char {
				score += scoring.matching_case_bonus;
			}
			if haystack_is_upper && previous_haystack_is_lower {
				score += scoring.capitalization_bonus;
			}
			if haystack_idx == 0 {
				score += scoring.prefix_bonus;
			} else if needle_idx == 0 && haystack_idx == 1 && !haystack[0].is_ascii_alphabetic() {
				score += scoring.offset_prefix_bonus;
			}
			if previous_haystack_is_delimiter && !haystack_is_delimiter {
				score += scoring.delimiter_bonus;
			}

			previous_haystack_is_delimiter = delimiter_bonus_enabled && haystack_is_delimiter;
			previous_haystack_is_lower = haystack_is_lower;

			indices.push(haystack_idx);
			haystack_idx += 1;
			continue 'outer;
		}

		// didn't find a match
		return (0, vec![], false);
	}

	let exact = haystack == needle;
	if exact {
		score += scoring.exact_match_bonus;
	}

	(score, indices, exact)
}

#[cfg(test)]
mod tests;
