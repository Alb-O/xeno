use super::Appendable;
use super::bucket::FixedWidthBucket;
use crate::engine::CandidateFilter;
use crate::limits::{exceeds_typo_budget, match_too_large};
use crate::smith_waterman::greedy::match_greedy;
use crate::{Config, Match};

/// Computes the Smith-Waterman score with affine gaps for a needle against a list of haystacks.
///
/// You should call this function with as many haystacks as you have available as it will
/// automatically chunk the haystacks based on string length to avoid unnecessary computation
/// due to SIMD
pub fn match_list<S1: AsRef<str>, S2: AsRef<str>>(needle: S1, haystacks: &[S2], config: &Config) -> Vec<Match> {
	let mut matches = if config.max_typos.is_none() {
		Vec::with_capacity(haystacks.len())
	} else {
		vec![]
	};

	match_list_impl(needle, haystacks, 0, config, &mut matches);

	if config.sort {
		#[cfg(feature = "parallel_sort")]
		{
			use rayon::prelude::*;
			matches.par_sort();
		}
		#[cfg(not(feature = "parallel_sort"))]
		matches.sort_unstable();
	}

	matches
}

pub(crate) fn match_list_impl<S1: AsRef<str>, S2: AsRef<str>, M: Appendable<Match>>(
	needle: S1,
	haystacks: &[S2],
	index_offset: u32,
	config: &Config,
	matches: &mut M,
) {
	assert!((index_offset as usize) + haystacks.len() < (u32::MAX as usize), "haystack index overflow");

	let needle = needle.as_ref();
	if needle.is_empty() {
		for (i, _) in haystacks.iter().enumerate() {
			matches.append(Match {
				index: (i as u32) + index_offset,
				score: 0,
				exact: false,
			});
		}
		return;
	}

	let filter = CandidateFilter::new(needle, config);

	macro_rules! run_with_buckets {
		($(($name:ident, $width:literal, $range:pat)),* $(,)?) => {
			$(let mut $name = FixedWidthBucket::<$width, M>::new(needle, config);)*

			for (i, haystack) in haystacks.iter().map(|h| h.as_ref()).enumerate() {
				if !filter.allows(haystack) {
					continue;
				}

				let i = i as u32 + index_offset;

				// fallback to greedy matching
				if match_too_large(needle, haystack) {
					let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
					if exceeds_typo_budget(config.max_typos, needle, indices.len()) {
						continue;
					}
					matches.append(Match { index: i, score, exact });
					continue;
				}

				// Pick the bucket to insert into based on the length of the haystack
				match haystack.len() {
					$($range => $name.add_haystack(matches, haystack, i),)*

					// fallback to greedy matching
					_ => {
						let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
						if exceeds_typo_budget(config.max_typos, needle, indices.len()) {
							continue;
						}
						matches.append(Match { index: i, score, exact });
						continue;
					}
				};
			}

			// Run processing on remaining haystacks in the buckets
			$($name.finalize(matches);)*
		};
	}
	crate::for_each_bucket_spec!(run_with_buckets);
}

#[cfg(test)]
mod tests;
