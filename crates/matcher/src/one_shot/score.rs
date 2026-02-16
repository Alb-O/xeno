use crate::Config;
use crate::engine::CandidateFilter;
use crate::limits::{exceeds_typo_budget, match_too_large, typo_sw_too_large};
use crate::smith_waterman::greedy::match_greedy;
use crate::smith_waterman::simd::{smith_waterman_scores, smith_waterman_scores_typos};

/// Reusable scorer for single-haystack scoring via SIMD (L=1).
///
/// Amortizes `CandidateFilter` construction across multiple haystacks.
/// Use this when you need scores for individual haystacks (e.g. top-K heap
/// scan) without the overhead of `match_indices` or `match_list`.
pub struct ScoreMatcher<'a> {
	needle: &'a str,
	config: &'a Config,
	filter: CandidateFilter,
}

impl<'a> ScoreMatcher<'a> {
	pub fn new(needle: &'a str, config: &'a Config) -> Self {
		let filter = CandidateFilter::new(needle, config);
		Self { needle, config, filter }
	}

	/// Scores a single haystack against the needle.
	///
	/// Returns `Some((score, exact))` if the haystack passes prefilter and typo
	/// gating, `None` otherwise. Empty needle matches everything with score 0.
	pub fn score(&self, haystack: &str) -> Option<(u16, bool)> {
		score_single(self.needle, haystack, self.config, &self.filter)
	}
}

/// Convenience wrapper: scores a single haystack without reusing filter state.
pub fn match_score(needle: &str, haystack: &str, config: &Config) -> Option<(u16, bool)> {
	if needle.is_empty() {
		return Some((0, false));
	}
	let filter = CandidateFilter::new(needle, config);
	score_single(needle, haystack, config, &filter)
}

fn score_single(needle: &str, haystack: &str, config: &Config, filter: &CandidateFilter) -> Option<(u16, bool)> {
	if needle.is_empty() {
		return Some((0, false));
	}

	if !filter.allows(haystack) {
		return None;
	}

	if match_too_large(needle, haystack) {
		let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
		if exceeds_typo_budget(config.max_typos, needle, indices.len()) {
			return None;
		}
		return Some((score, exact));
	}

	let len = haystack.len();

	macro_rules! dispatch_bucket {
		($(($name:ident, $width:literal, $range:pat)),* $(,)?) => {
			match len {
				$($range => score_with_width::<$width>(needle, haystack, config),)*
				_ => {
					let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
					if exceeds_typo_budget(config.max_typos, needle, indices.len()) {
						return None;
					}
					Some((score, exact))
				}
			}
		};
	}
	crate::for_each_bucket_spec!(dispatch_bucket)
}

fn score_with_width<const W: usize>(needle: &str, haystack: &str, config: &Config) -> Option<(u16, bool)> {
	match config.max_typos {
		None => {
			let (scores, exact) = smith_waterman_scores::<W, 1>(needle, &[haystack], &config.scoring);
			Some((scores[0], exact[0]))
		}
		Some(max_typos) => {
			if typo_sw_too_large(needle, W) {
				let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
				if exceeds_typo_budget(Some(max_typos), needle, indices.len()) {
					return None;
				}
				return Some((score, exact));
			}

			let (scores, typos, exact) = smith_waterman_scores_typos::<W, 1>(needle, &[haystack], max_typos, &config.scoring);
			if typos[0] > max_typos {
				return None;
			}
			Some((scores[0], exact[0]))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::one_shot::indices::match_indices;

	fn config_with_typos(max_typos: Option<u16>) -> Config {
		Config {
			max_typos,
			sort: false,
			..Config::default()
		}
	}

	/// `match_score` agrees with `match_indices` on score, exact, and Some/None.
	#[test]
	fn match_score_agrees_with_match_indices() {
		let cases = [
			("a", "abc"),
			("abc", "abc"),
			("deadbeef", "deadbeef"),
			("dead", "xdeadbeef"),
			("xyz", "abc"),
			("foo", "f_o_o"),
			("AB", "aAbBc"),
			("solf", "self::"),
			("", "anything"),
			("abc", "ab"),
		];

		for &max_typos in &[None, Some(0), Some(1), Some(2)] {
			let config = config_with_typos(max_typos);
			for &(needle, haystack) in &cases {
				let indices_result = match_indices(needle, haystack, &config);
				let score_result = match_score(needle, haystack, &config);

				match (&indices_result, &score_result) {
					(Some(mi), Some((score, exact))) => {
						assert_eq!(
							mi.score, *score,
							"score mismatch: needle={needle:?} haystack={haystack:?} max_typos={max_typos:?}"
						);
						assert_eq!(
							mi.exact, *exact,
							"exact mismatch: needle={needle:?} haystack={haystack:?} max_typos={max_typos:?}"
						);
					}
					(None, None) => {}
					_ => {
						panic!(
							"Some/None mismatch: needle={needle:?} haystack={haystack:?} max_typos={max_typos:?} indices={indices_result:?} score={score_result:?}"
						);
					}
				}
			}
		}
	}

	/// ScoreMatcher produces same results as match_score.
	#[test]
	fn score_matcher_parity() {
		let config = config_with_typos(Some(1));
		let scorer = ScoreMatcher::new("dead", &config);

		for haystack in &["deadbeef", "xdeadx", "abc", "dead", "d_e_a_d"] {
			assert_eq!(
				scorer.score(haystack),
				match_score("dead", haystack, &config),
				"parity mismatch for haystack={haystack:?}"
			);
		}
	}
}
