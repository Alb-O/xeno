use core::simd::Simd;
use core::simd::cmp::SimdOrd;

use crate::kernels::fixed_width::emit_fixed_width_matches;
use crate::simd_lanes::{LaneCount, SupportedLaneCount};
use core::simd::Mask;
use crate::smith_waterman::simd::{HaystackChar, NeedleChar, delimiter_masks, smith_waterman_inner, valid_masks};
use crate::{Match, Scoring};

pub(crate) struct IncrementalBucket<'a, const W: usize, const L: usize>
where
	LaneCount<L>: SupportedLaneCount,
{
	pub length: usize,
	pub idxs: [u32; L],
	pub haystack_strs: [&'a str; L],
	pub haystacks: [HaystackChar<L>; W],
	pub haystack_valid_masks: [Mask<i16, L>; W],
	pub score_matrix: Vec<[Simd<u16, L>; W]>,
}

impl<'a, const W: usize, const L: usize> IncrementalBucket<'a, W, L>
where
	LaneCount<L>: SupportedLaneCount,
{
	pub fn new(haystacks: &[&'a str; L], idxs: [u32; L], length: usize) -> Self {
		Self {
			length,
			idxs,
			haystack_strs: *haystacks,
			haystacks: std::array::from_fn(|i| HaystackChar::from_haystack(haystacks, i)),
			haystack_valid_masks: valid_masks::<W, L>(haystacks),
			score_matrix: vec![],
		}
	}
}

impl<'a, const W: usize, const L: usize> IncrementalBucket<'a, W, L>
where
	LaneCount<L>: SupportedLaneCount,
{
	#[inline]
	pub fn process(&mut self, prefix_to_keep: usize, needle: &str, matches: &mut Vec<Match>, max_typos: Option<u16>, scoring: &Scoring) {
		if let Some(max_typos) = max_typos {
			self.score_matrix.clear();
			emit_fixed_width_matches::<W, L>(needle, &self.haystack_strs, &self.idxs, self.length, Some(max_typos), scoring, |m| {
				matches.push(m)
			});
			return;
		}

		let new_needle_chars = needle.as_bytes()[prefix_to_keep..]
			.iter()
			.map(|&x| NeedleChar::new(x.into()))
			.collect::<Box<[_]>>();
		let target_len = needle.len();

		// Adjust score matrix to the new size
		if target_len > self.score_matrix.len() {
			self.score_matrix
				.extend(std::iter::repeat_n([Simd::splat(0); W], target_len - self.score_matrix.len()));
		} else if target_len < self.score_matrix.len() {
			self.score_matrix.truncate(target_len);
		}

		let mut ignored_max_score = Simd::splat(0);
		let haystack_delimiter_masks = delimiter_masks(&self.haystacks, scoring.delimiters.as_bytes());
		for (i, &needle_char) in new_needle_chars.iter().enumerate() {
			let needle_idx = i + prefix_to_keep;

			let (prev_score_col, curr_score_col) = if needle_idx == 0 {
				(None, self.score_matrix[needle_idx].as_mut())
			} else {
				let (a, b) = self.score_matrix.split_at_mut(needle_idx);
				(Some(a[needle_idx - 1].as_ref()), b[0].as_mut())
			};

			smith_waterman_inner(
				0,
				W,
				needle_char,
				&self.haystacks,
				haystack_delimiter_masks.as_slice(),
				Some(self.haystack_valid_masks.as_slice()),
				prev_score_col,
				curr_score_col,
				scoring,
				&mut ignored_max_score,
			);
		}

		let mut all_time_max_score = Simd::splat(0);
		for score_col in self.score_matrix.iter() {
			for score in score_col {
				all_time_max_score = score.simd_max(all_time_max_score);
			}
		}
		let scores: [u16; L] = all_time_max_score.to_array();

		#[allow(clippy::needless_range_loop)]
		for idx in 0..self.length {
			let score_idx = self.idxs[idx];
			let exact = self.haystack_strs[idx] == needle;
			let score = if exact {
				scores[idx].saturating_add(scoring.exact_match_bonus)
			} else {
				scores[idx]
			};

			matches.push(Match {
				index: score_idx,
				score,
				exact,
			});
		}
	}
}
