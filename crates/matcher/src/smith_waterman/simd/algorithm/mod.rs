//! SIMD Smith-Waterman scoring kernels and mask helpers.

use core::simd::cmp::*;
use core::simd::num::SimdUint;
use core::simd::{Mask, Simd};
use std::ops::Not;

use multiversion::multiversion;

use super::{HaystackChar, NeedleChar, interleave};
use crate::Scoring;
use crate::simd_lanes::{LaneCount, SupportedLaneCount};

#[inline(always)]
fn delimiter_mask<const L: usize>(lowercase: Simd<u16, L>, delimiters: &[u8]) -> Mask<i16, L>
where
	LaneCount<L>: SupportedLaneCount,
{
	delimiters.iter().fold(Mask::splat(false), |mask, delimiter| {
		mask | Simd::splat(delimiter.to_ascii_lowercase() as u16).simd_eq(lowercase)
	})
}

#[inline(always)]
pub(crate) fn delimiter_masks<const W: usize, const L: usize>(haystack: &[HaystackChar<L>; W], delimiters: &[u8]) -> [Mask<i16, L>; W]
where
	LaneCount<L>: SupportedLaneCount,
{
	std::array::from_fn(|idx| delimiter_mask(haystack[idx].lowercase, delimiters))
}

/// Per-position validity mask: `true` for lanes where `haystack_idx < haystack_len`.
/// Prevents score leakage into zero-padded positions beyond actual haystack length.
#[inline(always)]
pub(crate) fn valid_masks<const W: usize, const L: usize>(haystack_strs: &[&str; L]) -> [Mask<i16, L>; W]
where
	LaneCount<L>: SupportedLaneCount,
{
	let lens: [u16; L] = std::array::from_fn(|i| haystack_strs[i].len().min(W) as u16);
	let lens_simd = Simd::from_array(lens);
	std::array::from_fn(|j| Simd::splat(j as u16).simd_lt(lens_simd))
}

#[inline(always)]
#[allow(
	clippy::too_many_arguments,
	reason = "hot-path SIMD kernel uses explicit scalar/slice args to avoid packing overhead"
)]
pub(crate) fn smith_waterman_inner<const L: usize>(
	start: usize,
	end: usize,
	needle_char: NeedleChar<L>,
	haystack: &[HaystackChar<L>],
	haystack_delimiter_mask: &[Mask<i16, L>],
	haystack_valid_mask: Option<&[Mask<i16, L>]>,
	prev_score_col: Option<&[Simd<u16, L>]>,
	curr_score_col: &mut [Simd<u16, L>],
	scoring: &Scoring,
) where
	LaneCount<L>: SupportedLaneCount,
{
	let mut up_score_simd = Simd::splat(0);
	let mut up_gap_penalty_mask = Mask::splat(true);
	let mut left_gap_penalty_mask = Mask::splat(true);
	let mut delimiter_bonus_enabled_mask = if start == 0 {
		Mask::splat(false)
	} else {
		// Delimiter bonus is enabled after any non-delimiter has appeared in the row.
		// In banded runs (start > 0), seed from the skipped prefix so row-local
		// bonus behavior matches the full-width traversal.
		let mut seen_non_delimiter_mask = Mask::splat(false);
		let seed_end = start.min(haystack_delimiter_mask.len());
		for &is_delimiter_mask in &haystack_delimiter_mask[..seed_end] {
			seen_non_delimiter_mask |= is_delimiter_mask.not();
		}
		seen_non_delimiter_mask
	};

	for haystack_idx in start..end {
		let haystack_char = haystack[haystack_idx];
		let haystack_is_delimiter_mask = haystack_delimiter_mask[haystack_idx];

		let (diag, left) = if haystack_idx == 0 {
			(Simd::splat(0), prev_score_col.map(|c| c[0]).unwrap_or(Simd::splat(0)))
		} else {
			prev_score_col
				.map(|c| (c[haystack_idx - 1], c[haystack_idx]))
				.unwrap_or((Simd::splat(0), Simd::splat(0)))
		};

		// Calculate diagonal (match/mismatch) scores
		let match_mask: Mask<i16, L> = needle_char.lowercase.simd_eq(haystack_char.lowercase);
		let matched_casing_mask: Mask<i16, L> = needle_char.is_capital_mask.simd_eq(haystack_char.is_capital_mask);

		let match_score = if haystack_idx > 0 {
			let match_score = {
				let prev_haystack_char = haystack[haystack_idx - 1];
				let prev_haystack_is_delimiter_mask = haystack_delimiter_mask[haystack_idx - 1];

				// ignore capitalization on the prefix
				let capitalization_bonus_mask: Mask<i16, L> = haystack_char.is_capital_mask & prev_haystack_char.is_lower_mask;
				let capitalization_bonus = capitalization_bonus_mask.select(Simd::splat(scoring.capitalization_bonus), Simd::splat(0));

				let delimiter_bonus_mask: Mask<i16, L> = prev_haystack_is_delimiter_mask & delimiter_bonus_enabled_mask & !haystack_is_delimiter_mask;
				let delimiter_bonus = delimiter_bonus_mask.select(Simd::splat(scoring.delimiter_bonus), Simd::splat(0));

				capitalization_bonus + delimiter_bonus + Simd::splat(scoring.match_score)
			};

			if haystack_idx == 1 {
				// If the first char is not a letter, apply the offset prefix bonus on the second char
				// if we didn't match on the first char
				// I.e. `a` matching on `-a` would get the offset prefix bonus
				// but  `b` matching on `ab` would not get the offset prefix bonus
				let offset_prefix_mask = !(haystack[0].is_lower_mask | haystack[0].is_capital_mask) & diag.simd_eq(Simd::splat(0));

				offset_prefix_mask.select(Simd::splat(scoring.offset_prefix_bonus + scoring.match_score), match_score)
			} else {
				match_score
			}
		} else {
			// Give a bonus for prefix matches
			Simd::splat(scoring.prefix_bonus + scoring.match_score)
		};

		let diag_score = match_mask.select(
			diag + matched_casing_mask.select(Simd::splat(scoring.matching_case_bonus), Simd::splat(0)) + match_score,
			diag.saturating_sub(Simd::splat(scoring.mismatch_penalty)),
		);

		// Load and calculate up scores (skipping char in haystack)
		let up_gap_penalty = up_gap_penalty_mask.select(Simd::splat(scoring.gap_open_penalty), Simd::splat(scoring.gap_extend_penalty));
		let up_score = up_score_simd.saturating_sub(up_gap_penalty);

		// Load and calculate left scores (skipping char in needle)
		let left_gap_penalty = left_gap_penalty_mask.select(Simd::splat(scoring.gap_open_penalty), Simd::splat(scoring.gap_extend_penalty));
		let left_score = left.saturating_sub(left_gap_penalty);

		// Calculate maximum scores
		let max_score = diag_score.simd_max(up_score).simd_max(left_score);

		// Update gap penalty mask
		let diag_mask: Mask<i16, L> = max_score.simd_eq(diag_score);
		up_gap_penalty_mask = max_score.simd_ne(up_score) | diag_mask;
		left_gap_penalty_mask = max_score.simd_ne(left_score) | diag_mask;

		// Only enable delimiter bonus if we've seen a non-delimiter char
		delimiter_bonus_enabled_mask |= haystack_is_delimiter_mask.not();

		// Zero out padded lanes (haystacks shorter than W)
		let max_score = if let Some(mask) = haystack_valid_mask {
			mask[haystack_idx].select(max_score, Simd::splat(0))
		} else {
			max_score
		};

		// Store the scores for the next iterations
		up_score_simd = max_score;
		curr_score_col[haystack_idx] = max_score;
	}
}

#[multiversion(targets(
    // x86-64-v4 without lahfsahf
    "x86_64+avx512f+avx512bw+avx512cd+avx512dq+avx512vl+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v3 without lahfsahf
    "x86_64+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v2 without lahfsahf
    "x86_64+cmpxchg16b+fxsr+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3",
))]
pub fn smith_waterman_scores<const W: usize, const L: usize>(needle_str: &str, haystack_strs: &[&str; L], scoring: &Scoring) -> ([u16; L], [bool; L])
where
	LaneCount<L>: SupportedLaneCount,
{
	let needle = needle_str.as_bytes();
	let haystacks = interleave::<W, L>(*haystack_strs).map(HaystackChar::new);
	let delimiters = scoring.delimiters.as_bytes();
	let haystack_delimiter_mask = delimiter_masks(&haystacks, delimiters);
	let needs_mask = haystack_strs.iter().any(|s| s.len() < W);
	let haystack_valid_mask = if needs_mask { Some(valid_masks::<W, L>(haystack_strs)) } else { None };

	let mut prev_score_col = [Simd::splat(0); W];
	let mut curr_score_col = [Simd::splat(0); W];

	for (needle_idx, &needle_byte) in needle.iter().enumerate() {
		let needle_char = NeedleChar::new(needle_byte as u16);
		let prev_col = if needle_idx == 0 { None } else { Some(prev_score_col.as_slice()) };

		smith_waterman_inner(
			0,
			W,
			needle_char,
			&haystacks,
			haystack_delimiter_mask.as_slice(),
			haystack_valid_mask.as_ref().map(|m| m.as_slice()),
			prev_col,
			curr_score_col.as_mut_slice(),
			scoring,
		);

		std::mem::swap(&mut prev_score_col, &mut curr_score_col);
	}

	// Full-needle contract: score is the best alignment that consumes the whole needle,
	// i.e. max over the last needle row (prev_score_col after final swap).
	let last_row_max = if needle.is_empty() {
		Simd::splat(0)
	} else {
		prev_score_col.iter().copied().reduce(|a, b| a.simd_max(b)).unwrap_or(Simd::splat(0))
	};

	let exact_matches: [bool; L] = std::array::from_fn(|i| haystack_strs[i] == needle_str);
	let max_scores = std::array::from_fn(|i| {
		let mut score = last_row_max[i];
		if exact_matches[i] {
			score += scoring.exact_match_bonus;
		}
		score
	});

	(max_scores, exact_matches)
}

#[multiversion(targets(
    // x86-64-v4 without lahfsahf
    "x86_64+avx512f+avx512bw+avx512cd+avx512dq+avx512vl+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v3 without lahfsahf
    "x86_64+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v2 without lahfsahf
    "x86_64+cmpxchg16b+fxsr+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3",
))]
/// Computes Smith-Waterman scores and typo counts via streaming forward DP.
///
/// Full-needle contract: score is the best alignment consuming the whole needle (max
/// over the last needle row). Typo count = `needle_len - matched_chars`, where
/// `matched_chars` is the number of diagonal-match steps on the chosen path.
///
/// Tracks `matched_count` per cell alongside scores using the same winner selection
/// as `smith_waterman_inner`. On diagonal match steps, matched_count increments;
/// all other transitions carry their predecessor's matched_count unchanged.
/// This implementation avoids materializing the score matrix or running traceback:
/// it streams DP columns and carries matched-count state in lockstep with score
/// winner selection.
///
/// `_max_typos` is retained for API compatibility and is not used in this path.
pub fn smith_waterman_scores_typos<const W: usize, const L: usize>(
	needle_str: &str,
	haystack_strs: &[&str; L],
	_max_typos: u16,
	scoring: &Scoring,
) -> ([u16; L], [u16; L], [bool; L])
where
	LaneCount<L>: SupportedLaneCount,
{
	let needle = needle_str.as_bytes();
	let haystacks = interleave::<W, L>(*haystack_strs).map(HaystackChar::new);
	let delimiters = scoring.delimiters.as_bytes();
	let haystack_delimiter_mask = delimiter_masks(&haystacks, delimiters);
	let needs_mask = haystack_strs.iter().any(|s| s.len() < W);
	let haystack_valid_mask = if needs_mask { Some(valid_masks::<W, L>(haystack_strs)) } else { None };

	let mut prev_score_col = [Simd::<u16, L>::splat(0); W];
	let mut curr_score_col = [Simd::<u16, L>::splat(0); W];
	let mut prev_matched_col = [Simd::<u16, L>::splat(0); W];
	let mut curr_matched_col = [Simd::<u16, L>::splat(0); W];

	for (needle_idx, &needle_byte) in needle.iter().enumerate() {
		let needle_char = NeedleChar::new(needle_byte as u16);

		let mut up_score_simd = Simd::splat(0);
		let mut up_matched_simd = Simd::splat(0);
		let mut up_gap_penalty_mask = Mask::splat(true);
		let mut left_gap_penalty_mask = Mask::splat(true);
		let mut delimiter_bonus_enabled_mask = Mask::<i16, L>::splat(false);

		for haystack_idx in 0..W {
			let haystack_char = haystacks[haystack_idx];
			let haystack_is_delimiter_mask = haystack_delimiter_mask[haystack_idx];

			// Load predecessors: (score, matched_count) for diagonal, left, and up paths
			let (diag_score_prev, diag_matched_prev, left_score_prev, left_matched_prev) = if haystack_idx == 0 {
				let left = if needle_idx == 0 { Simd::splat(0) } else { prev_score_col[0] };
				let left_matched = if needle_idx == 0 { Simd::splat(0) } else { prev_matched_col[0] };
				// Boundary diagonal: no previous alignment, matched_count = 0
				(Simd::splat(0), Simd::splat(0), left, left_matched)
			} else if needle_idx == 0 {
				(Simd::splat(0), Simd::splat(0), Simd::splat(0), Simd::splat(0))
			} else {
				(
					prev_score_col[haystack_idx - 1],
					prev_matched_col[haystack_idx - 1],
					prev_score_col[haystack_idx],
					prev_matched_col[haystack_idx],
				)
			};

			// Calculate diagonal (match/mismatch) scores — same logic as smith_waterman_inner
			let match_mask: Mask<i16, L> = needle_char.lowercase.simd_eq(haystack_char.lowercase);
			let matched_casing_mask: Mask<i16, L> = needle_char.is_capital_mask.simd_eq(haystack_char.is_capital_mask);

			let match_score = if haystack_idx > 0 {
				let base_match_score = {
					let prev_haystack_char = haystacks[haystack_idx - 1];
					let prev_haystack_is_delimiter_mask = haystack_delimiter_mask[haystack_idx - 1];

					let capitalization_bonus_mask: Mask<i16, L> = haystack_char.is_capital_mask & prev_haystack_char.is_lower_mask;
					let capitalization_bonus = capitalization_bonus_mask.select(Simd::splat(scoring.capitalization_bonus), Simd::splat(0));

					let delimiter_bonus_mask: Mask<i16, L> = prev_haystack_is_delimiter_mask & delimiter_bonus_enabled_mask & !haystack_is_delimiter_mask;
					let delimiter_bonus = delimiter_bonus_mask.select(Simd::splat(scoring.delimiter_bonus), Simd::splat(0));

					capitalization_bonus + delimiter_bonus + Simd::splat(scoring.match_score)
				};

				if haystack_idx == 1 {
					let offset_prefix_mask = !(haystacks[0].is_lower_mask | haystacks[0].is_capital_mask) & diag_score_prev.simd_eq(Simd::splat(0));
					offset_prefix_mask.select(Simd::splat(scoring.offset_prefix_bonus + scoring.match_score), base_match_score)
				} else {
					base_match_score
				}
			} else {
				Simd::splat(scoring.prefix_bonus + scoring.match_score)
			};

			let diag_score = match_mask.select(
				diag_score_prev + matched_casing_mask.select(Simd::splat(scoring.matching_case_bonus), Simd::splat(0)) + match_score,
				diag_score_prev.saturating_sub(Simd::splat(scoring.mismatch_penalty)),
			);

			// Up score (skip haystack char)
			let up_gap_penalty = up_gap_penalty_mask.select(Simd::splat(scoring.gap_open_penalty), Simd::splat(scoring.gap_extend_penalty));
			let up_score = up_score_simd.saturating_sub(up_gap_penalty);

			// Left score (skip needle char)
			let left_gap_penalty = left_gap_penalty_mask.select(Simd::splat(scoring.gap_open_penalty), Simd::splat(scoring.gap_extend_penalty));
			let left_score = left_score_prev.saturating_sub(left_gap_penalty);

			// Winner selection: same as smith_waterman_inner (score-based, diag wins ties)
			let max_score = diag_score.simd_max(up_score).simd_max(left_score);
			let diag_wins: Mask<i16, L> = max_score.simd_eq(diag_score);
			let up_wins: Mask<i16, L> = max_score.simd_eq(up_score) & !diag_wins;

			// Matched-count tracking:
			// diagonal match → matched + 1; all other transitions → carry predecessor's count
			let diag_matched = diag_matched_prev + match_mask.select(Simd::splat(1), Simd::splat(0));
			let up_matched = up_matched_simd;
			let left_matched = left_matched_prev;

			let max_matched = diag_wins.select(diag_matched, up_wins.select(up_matched, left_matched));

			// Update gap penalty masks (same logic as smith_waterman_inner)
			let diag_mask = max_score.simd_eq(diag_score);
			up_gap_penalty_mask = max_score.simd_ne(up_score) | diag_mask;
			left_gap_penalty_mask = max_score.simd_ne(left_score) | diag_mask;

			delimiter_bonus_enabled_mask |= haystack_is_delimiter_mask.not();

			// Zero out padded lanes
			let (max_score, max_matched) = if let Some(mask) = &haystack_valid_mask {
				(
					mask[haystack_idx].select(max_score, Simd::splat(0)),
					mask[haystack_idx].select(max_matched, Simd::splat(0)),
				)
			} else {
				(max_score, max_matched)
			};

			up_score_simd = max_score;
			up_matched_simd = max_matched;
			curr_score_col[haystack_idx] = max_score;
			curr_matched_col[haystack_idx] = max_matched;
		}

		std::mem::swap(&mut prev_score_col, &mut curr_score_col);
		std::mem::swap(&mut prev_matched_col, &mut curr_matched_col);
	}

	// Full-needle contract: score and matched from last needle row argmax.
	let needle_len = Simd::splat(needle.len() as u16);
	let (last_row_scores, last_row_typos) = if needle.is_empty() {
		(Simd::splat(0), Simd::splat(0))
	} else {
		let mut best_scores = prev_score_col[0];
		let mut best_matched = prev_matched_col[0];
		for j in 1..W {
			let better: Mask<i16, L> = prev_score_col[j].simd_gt(best_scores);
			best_scores = better.select(prev_score_col[j], best_scores);
			best_matched = better.select(prev_matched_col[j], best_matched);
		}
		// typos = needle_len - matched_chars
		let typos = needle_len - best_matched.simd_min(needle_len);
		(best_scores, typos)
	};

	let exact_matches: [bool; L] = std::array::from_fn(|i| haystack_strs[i] == needle_str);
	let max_scores = std::array::from_fn(|i| {
		let mut score = last_row_scores[i];
		if exact_matches[i] {
			score += scoring.exact_match_bonus;
		}
		score
	});
	let typos: [u16; L] = last_row_typos.to_array();

	(max_scores, typos, exact_matches)
}

/// Materializes the full score matrix for testing and traceback. Supports optional banding
/// via `max_typos` for performance testing, but production code should use
/// [`smith_waterman_scores`] or [`smith_waterman_scores_typos`] instead.
#[cfg(test)]
#[multiversion(targets(
    // x86-64-v4 without lahfsahf
    "x86_64+avx512f+avx512bw+avx512cd+avx512dq+avx512vl+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v3 without lahfsahf
    "x86_64+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v2 without lahfsahf
    "x86_64+cmpxchg16b+fxsr+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3",
))]
pub fn smith_waterman<const W: usize, const L: usize>(
	needle_str: &str,
	haystack_strs: &[&str; L],
	max_typos: Option<u16>,
	scoring: &Scoring,
) -> ([u16; L], Vec<[Simd<u16, L>; W]>, [bool; L])
where
	LaneCount<L>: SupportedLaneCount,
{
	let needle = needle_str.as_bytes();
	let haystacks = interleave::<W, L>(*haystack_strs).map(HaystackChar::new);
	let delimiters = scoring.delimiters.as_bytes();
	let haystack_delimiter_mask = delimiter_masks(&haystacks, delimiters);
	let needs_mask = haystack_strs.iter().any(|s| s.len() < W);
	let haystack_valid_mask = if needs_mask { Some(valid_masks::<W, L>(haystack_strs)) } else { None };

	// State
	let mut score_matrix = vec![[Simd::splat(0); W]; needle.len()];

	for (needle_idx, haystack_start, haystack_end) in (0..needle.len()).map(|needle_idx| {
		let haystack_start = max_typos.map(|max_typos| needle_idx.saturating_sub(max_typos as usize)).unwrap_or(0);
		let haystack_end = max_typos
			.map(|max_typos| (W + needle_idx + (max_typos as usize)).saturating_sub(needle.len()).min(W))
			.unwrap_or(W);
		(needle_idx, haystack_start, haystack_end)
	}) {
		let needle_char = NeedleChar::new(needle[needle_idx] as u16);

		let (prev_score_col, curr_score_col) = if needle_idx == 0 {
			(None, &mut score_matrix[needle_idx])
		} else {
			let (a, b) = score_matrix.split_at_mut(needle_idx);
			(Some(a[needle_idx - 1].as_slice()), &mut b[0])
		};

		smith_waterman_inner(
			haystack_start,
			haystack_end,
			needle_char,
			&haystacks,
			haystack_delimiter_mask.as_slice(),
			haystack_valid_mask.as_ref().map(|m| m.as_slice()),
			prev_score_col,
			curr_score_col,
			scoring,
		);
	}

	// Full-needle contract: score is max over the last needle row.
	let last_row_max = if needle.is_empty() {
		Simd::splat(0)
	} else {
		score_matrix
			.last()
			.unwrap()
			.iter()
			.copied()
			.reduce(|a, b| a.simd_max(b))
			.unwrap_or(Simd::splat(0))
	};

	let exact_matches: [bool; L] = std::array::from_fn(|i| haystack_strs[i] == needle_str);

	let max_scores = std::array::from_fn(|i| {
		let mut score = last_row_max[i];
		if exact_matches[i] {
			score += scoring.exact_match_bonus;
		}
		score
	});

	(max_scores, score_matrix, exact_matches)
}

#[cfg(test)]
mod tests;
