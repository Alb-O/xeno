use core::simd::prelude::*;
use core::simd::{Mask, Simd};

use multiversion::multiversion;

use crate::simd_lanes::{LaneCount, SupportedLaneCount};

#[multiversion(targets(
    // x86-64-v4 without lahfsahf
    "x86_64+avx512f+avx512bw+avx512cd+avx512dq+avx512vl+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v3 without lahfsahf
    "x86_64+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v2 without lahfsahf
    "x86_64+cmpxchg16b+fxsr+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3",
))]
pub fn typos_from_score_matrix<const W: usize, const L: usize>(score_matrix: &[[Simd<u16, L>; W]], max_typos: u16) -> [u16; L]
where
	LaneCount<L>: SupportedLaneCount,
{
	let mut typo_count = [0u16; L];
	let mut scores = Simd::splat(0);
	let mut positions = Simd::splat(0);

	// Get the starting position by looking at the last column
	// (last character of the needle)
	let last_column = score_matrix.last().unwrap();
	for (idx, &row_scores) in last_column.iter().enumerate() {
		let row_max_mask: Mask<i16, L> = row_scores.simd_gt(scores);
		scores = row_max_mask.select(row_scores, scores);
		positions = row_max_mask.select(Simd::splat(idx as u16), positions);
	}

	// Traceback and store the matched indices
	for (idx, &row_idx) in positions.to_array().iter().enumerate() {
		let mut col_idx = score_matrix.len() - 1;
		let mut row_idx: usize = row_idx.into();
		let mut score = scores[idx];

		// NOTE: row_idx = 0 or col_idx = 0 will always have a score of 0
		while col_idx > 0 {
			if typo_count[idx] > max_typos {
				break;
			}

			// Must be moving left
			if row_idx == 0 {
				typo_count[idx] += 1;
				col_idx -= 1;
				continue;
			}

			// Gather up the scores for all possible paths
			let diag = score_matrix[col_idx - 1][row_idx - 1][idx];
			let left = score_matrix[col_idx - 1][row_idx][idx];
			let up = score_matrix[col_idx][row_idx - 1][idx];

			// Match or mismatch
			if diag >= left && diag >= up {
				// Must be a mismatch
				if diag >= score {
					typo_count[idx] += 1;
				}
				row_idx -= 1;
				col_idx -= 1;
				score = diag;
			// Skipped character in needle
			} else if left >= up {
				typo_count[idx] += 1;
				col_idx -= 1;
				score = left;
			// Skipped character in haystack
			} else {
				row_idx -= 1;
				score = up;
			}
		}

		// HACK: Compensate for the last column being a typo
		if col_idx == 0 && score == 0 {
			typo_count[idx] += 1;
		}
	}

	typo_count
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Scoring;
	use crate::smith_waterman::simd::{smith_waterman, smith_waterman_scores_typos};

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

	fn gen_ascii(rng: &mut XorShift64, len: usize) -> String {
		const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_-/";
		let mut out = Vec::with_capacity(len);
		for _ in 0..len {
			out.push(ALPHABET[rng.next_usize(ALPHABET.len())]);
		}
		String::from_utf8(out).expect("generated string is valid ASCII")
	}

	fn get_typos(needle: &str, haystack: &str) -> u16 {
		typos_from_score_matrix(&smith_waterman::<4, 1>(needle, &[haystack; 1], None, &Scoring::default()).1, 100)[0]
	}

	#[test]
	fn test_typos_basic() {
		assert_eq!(get_typos("a", "abc"), 0);
		assert_eq!(get_typos("b", "abc"), 0);
		assert_eq!(get_typos("c", "abc"), 0);
		assert_eq!(get_typos("ac", "abc"), 0);

		assert_eq!(get_typos("d", "abc"), 1);
		assert_eq!(get_typos("da", "abc"), 1);
		assert_eq!(get_typos("dc", "abc"), 1);
		assert_eq!(get_typos("ad", "abc"), 1);
		assert_eq!(get_typos("adc", "abc"), 1);
		assert_eq!(get_typos("add", "abc"), 2);
		assert_eq!(get_typos("ddd", "abc"), 3);
		assert_eq!(get_typos("ddd", ""), 3);
		assert_eq!(get_typos("d", ""), 1);
	}

	#[test]
	fn streaming_typos_matches_matrix_traceback() {
		const W: usize = 32;
		const L: usize = 8;

		let scoring = Scoring::default();
		let mut rng = XorShift64::new(0xD1A4_93B5_77C2_1E0F);

		for max_typos in [0u16, 1u16] {
			for case_idx in 0..200 {
				let needle_len = 1 + rng.next_usize(12);
				let needle = gen_ascii(&mut rng, needle_len);

				let haystack_storage: [String; L] = std::array::from_fn(|_| {
					let len = rng.next_usize(W + 1);
					gen_ascii(&mut rng, len)
				});
				let haystacks: [&str; L] = std::array::from_fn(|i| haystack_storage[i].as_str());

				let (old_scores, score_matrix, old_exact) = smith_waterman::<W, L>(&needle, &haystacks, None, &scoring);
				let old_typos = typos_from_score_matrix::<W, L>(&score_matrix, max_typos);

				let (new_scores, new_typos, new_exact) = smith_waterman_scores_typos::<W, L>(&needle, &haystacks, max_typos, &scoring);

				assert_eq!(new_scores, old_scores, "score mismatch on case {case_idx} max_typos={max_typos}");
				assert_eq!(new_exact, old_exact, "exact mismatch on case {case_idx} max_typos={max_typos}");
				let old_within_budget = old_typos.map(|typo| typo <= max_typos);
				let new_within_budget = new_typos.map(|typo| typo <= max_typos);
				assert_eq!(
					new_within_budget, old_within_budget,
					"typo budget mismatch on case {case_idx} max_typos={max_typos} needle={needle:?} haystacks={haystacks:?}"
				);

				for lane_idx in 0..L {
					if old_typos[lane_idx] <= max_typos {
						assert_eq!(
							new_typos[lane_idx], old_typos[lane_idx],
							"in-budget typo mismatch lane {lane_idx} case {case_idx} max_typos={max_typos}"
						);
					}
				}
			}
		}
	}
}
