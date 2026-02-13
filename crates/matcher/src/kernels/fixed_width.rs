use crate::limits::{exceeds_typo_budget, typo_sw_too_large};
use crate::simd_lanes::{LaneCount, SupportedLaneCount};
use crate::smith_waterman::greedy::match_greedy;
use crate::smith_waterman::simd::{smith_waterman_scores, smith_waterman_scores_typos};
use crate::{Match, Scoring};

pub(crate) fn emit_fixed_width_matches<const W: usize, const L: usize>(
	needle: &str,
	haystacks: &[&str; L],
	idxs: &[u32; L],
	length: usize,
	max_typos: Option<u16>,
	scoring: &Scoring,
	mut emit: impl FnMut(Match),
) where
	LaneCount<L>: SupportedLaneCount,
{
	debug_assert!(length <= L);
	if length == 0 {
		return;
	}

	match max_typos {
		None => {
			let (scores, exact_matches) = smith_waterman_scores::<W, L>(needle, haystacks, scoring);
			for i in 0..length {
				emit(Match {
					index: idxs[i],
					score: scores[i],
					exact: exact_matches[i],
				});
			}
		}
		Some(max_typos) => {
			if typo_sw_too_large(needle, W) {
				for i in 0..length {
					let (score, indices, exact) = match_greedy(needle, haystacks[i], scoring);
					if exceeds_typo_budget(Some(max_typos), needle, indices.len()) {
						continue;
					}

					emit(Match { index: idxs[i], score, exact });
				}
				return;
			}

			let (scores, typos, exact_matches) = smith_waterman_scores_typos::<W, L>(needle, haystacks, max_typos, scoring);
			for i in 0..length {
				if typos[i] > max_typos {
					continue;
				}

				emit(Match {
					index: idxs[i],
					score: scores[i],
					exact: exact_matches[i],
				});
			}
		}
	}
}
