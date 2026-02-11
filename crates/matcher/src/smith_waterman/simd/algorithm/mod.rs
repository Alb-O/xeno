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

#[inline(always)]
pub(crate) fn smith_waterman_inner<const L: usize>(
	start: usize,
	end: usize,
	needle_char: NeedleChar<L>,
	haystack: &[HaystackChar<L>],
	haystack_delimiter_mask: &[Mask<i16, L>],
	prev_score_col: Option<&[Simd<u16, L>]>,
	curr_score_col: &mut [Simd<u16, L>],
	scoring: &Scoring,
	all_time_max_score: &mut Simd<u16, L>,
) where
	LaneCount<L>: SupportedLaneCount,
{
	let mut up_score_simd = Simd::splat(0);
	let mut up_gap_penalty_mask = Mask::splat(true);
	let mut left_gap_penalty_mask = Mask::splat(true);
	let mut delimiter_bonus_enabled_mask = Mask::splat(false);

	for haystack_idx in start..end {
		let haystack_char = haystack[haystack_idx];
		let haystack_is_delimiter_mask = haystack_delimiter_mask[haystack_idx];

		let (diag, left) = if haystack_idx == 0 {
			(Simd::splat(0), Simd::splat(0))
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

		// Store the scores for the next iterations
		up_score_simd = max_score;
		curr_score_col[haystack_idx] = max_score;
		*all_time_max_score = (*all_time_max_score).simd_max(max_score);
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

	let mut prev_score_col = [Simd::splat(0); W];
	let mut curr_score_col = [Simd::splat(0); W];
	let mut all_time_max_score = Simd::splat(0);

	for needle_idx in 0..needle.len() {
		let needle_char = NeedleChar::new(needle[needle_idx] as u16);
		let prev_col = if needle_idx == 0 { None } else { Some(prev_score_col.as_slice()) };

		smith_waterman_inner(
			0,
			W,
			needle_char,
			&haystacks,
			haystack_delimiter_mask.as_slice(),
			prev_col,
			curr_score_col.as_mut_slice(),
			scoring,
			&mut all_time_max_score,
		);

		std::mem::swap(&mut prev_score_col, &mut curr_score_col);
	}

	let exact_matches: [bool; L] = std::array::from_fn(|i| haystack_strs[i] == needle_str);
	let max_scores = std::array::from_fn(|i| {
		let mut score = all_time_max_score[i];
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

	// State
	let mut score_matrix = vec![[Simd::splat(0); W]; needle.len()];
	let mut all_time_max_score = Simd::splat(0);

	for (needle_idx, haystack_start, haystack_end) in (0..needle.len()).map(|needle_idx| {
		// When matching "asd" against "qwerty" with max_typos = 0, we can avoid matching "s"
		// against the "q" since it's impossible for this to be a valid match
		// And likewise, we avoid matching "d" against "q" and "w"
		let haystack_start = max_typos.map(|max_typos| needle_idx.saturating_sub(max_typos as usize)).unwrap_or(0);
		// When matching "foo" against "foobar" with max_typos = 0, we can avoid matching "f"
		// againt "a" and "r" since it's impossible for this to be a valid match
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
			prev_score_col,
			curr_score_col,
			scoring,
			&mut all_time_max_score,
		);
	}

	let exact_matches: [bool; L] = std::array::from_fn(|i| haystack_strs[i] == needle_str);

	let max_scores = std::array::from_fn(|i| {
		let mut score = all_time_max_score[i];
		if exact_matches[i] {
			score += scoring.exact_match_bonus;
		}
		score
	});

	(max_scores, score_matrix, exact_matches)
}

#[cfg(test)]
mod tests;
