use super::Appendable;
use super::bucket::FixedWidthBucket;
use crate::one_shot::{exceeds_typo_budget, match_too_large};
use crate::prefilter::Prefilter;
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

	let use_prefilter = config.prefilter && config.max_typos.is_some();
	let prefilter = use_prefilter.then(|| Prefilter::new(needle, config.max_typos.unwrap_or(0)));

	let mut bucket_size_4 = FixedWidthBucket::<4, M>::new(needle, config);
	let mut bucket_size_8 = FixedWidthBucket::<8, M>::new(needle, config);
	let mut bucket_size_12 = FixedWidthBucket::<12, M>::new(needle, config);
	let mut bucket_size_16 = FixedWidthBucket::<16, M>::new(needle, config);
	let mut bucket_size_20 = FixedWidthBucket::<20, M>::new(needle, config);
	let mut bucket_size_24 = FixedWidthBucket::<24, M>::new(needle, config);
	let mut bucket_size_32 = FixedWidthBucket::<32, M>::new(needle, config);
	let mut bucket_size_48 = FixedWidthBucket::<48, M>::new(needle, config);
	let mut bucket_size_64 = FixedWidthBucket::<64, M>::new(needle, config);
	let mut bucket_size_96 = FixedWidthBucket::<96, M>::new(needle, config);
	let mut bucket_size_128 = FixedWidthBucket::<128, M>::new(needle, config);
	let mut bucket_size_160 = FixedWidthBucket::<160, M>::new(needle, config);
	let mut bucket_size_192 = FixedWidthBucket::<192, M>::new(needle, config);
	let mut bucket_size_224 = FixedWidthBucket::<224, M>::new(needle, config);
	let mut bucket_size_256 = FixedWidthBucket::<256, M>::new(needle, config);
	let mut bucket_size_384 = FixedWidthBucket::<384, M>::new(needle, config);
	let mut bucket_size_512 = FixedWidthBucket::<512, M>::new(needle, config);

	// If max_typos is set, we can ignore any haystacks that are shorter than the needle
	// minus the max typos, since it's impossible for them to match
	let min_haystack_len = config.max_typos.map(|max| needle.len().saturating_sub(max as usize)).unwrap_or(0);

	for (i, haystack) in haystacks
		.iter()
		.map(|h| h.as_ref())
		.enumerate()
		.filter(|(_, h)| h.len() >= min_haystack_len)
		.filter(|(_, h)| {
			if !use_prefilter {
				return true;
			}

			let prefilter = prefilter.as_ref().expect("prefilter exists when enabled");
			match config.max_typos {
				Some(0) => prefilter.match_haystack_unordered_insensitive(h.as_bytes()),
				Some(_) => prefilter.match_haystack_unordered_typos_insensitive(h.as_bytes()),
				None => true,
			}
		}) {
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
			0..=4 => bucket_size_4.add_haystack(matches, haystack, i),
			5..=8 => bucket_size_8.add_haystack(matches, haystack, i),
			9..=12 => bucket_size_12.add_haystack(matches, haystack, i),
			13..=16 => bucket_size_16.add_haystack(matches, haystack, i),
			17..=20 => bucket_size_20.add_haystack(matches, haystack, i),
			21..=24 => bucket_size_24.add_haystack(matches, haystack, i),
			25..=32 => bucket_size_32.add_haystack(matches, haystack, i),
			33..=48 => bucket_size_48.add_haystack(matches, haystack, i),
			49..=64 => bucket_size_64.add_haystack(matches, haystack, i),
			65..=96 => bucket_size_96.add_haystack(matches, haystack, i),
			97..=128 => bucket_size_128.add_haystack(matches, haystack, i),
			129..=160 => bucket_size_160.add_haystack(matches, haystack, i),
			161..=192 => bucket_size_192.add_haystack(matches, haystack, i),
			193..=224 => bucket_size_224.add_haystack(matches, haystack, i),
			225..=256 => bucket_size_256.add_haystack(matches, haystack, i),
			257..=384 => bucket_size_384.add_haystack(matches, haystack, i),
			385..=512 => bucket_size_512.add_haystack(matches, haystack, i),

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
	bucket_size_4.finalize(matches);
	bucket_size_8.finalize(matches);
	bucket_size_12.finalize(matches);
	bucket_size_16.finalize(matches);
	bucket_size_20.finalize(matches);
	bucket_size_24.finalize(matches);
	bucket_size_32.finalize(matches);
	bucket_size_48.finalize(matches);
	bucket_size_64.finalize(matches);
	bucket_size_96.finalize(matches);
	bucket_size_128.finalize(matches);
	bucket_size_160.finalize(matches);
	bucket_size_192.finalize(matches);
	bucket_size_224.finalize(matches);
	bucket_size_256.finalize(matches);
	bucket_size_384.finalize(matches);
	bucket_size_512.finalize(matches);
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::match_list_parallel;

	#[test]
	fn test_basic() {
		let needle = "deadbe";
		let haystack = vec!["deadbeef", "deadbf", "deadbeefg", "deadbe"];

		let config = Config {
			max_typos: None,
			..Config::default()
		};
		let matches = match_list(needle, &haystack, &config);

		assert_eq!(matches.len(), 4);
		assert_eq!(matches[0].index, 3);
		assert_eq!(matches[1].index, 0);
		assert_eq!(matches[2].index, 2);
		assert_eq!(matches[3].index, 1);
	}

	#[test]
	fn test_no_typos() {
		let needle = "deadbe";
		let haystack = vec!["deadbeef", "deadbf", "deadbeefg", "deadbe"];

		let matches = match_list(
			needle,
			&haystack,
			&Config {
				max_typos: Some(0),
				..Config::default()
			},
		);
		assert_eq!(matches.len(), 3);
	}

	#[test]
	fn test_exact_match() {
		let needle = "deadbe";
		let haystack = vec!["deadbeef", "deadbf", "deadbeefg", "deadbe"];

		let matches = match_list(needle, &haystack, &Config::default());

		let exact_matches = matches.iter().filter(|m| m.exact).collect::<Vec<&Match>>();
		assert_eq!(exact_matches.len(), 1);
		assert_eq!(exact_matches[0].index, 3);
		for m in &exact_matches {
			assert_eq!(haystack[m.index as usize], needle)
		}
	}

	#[test]
	fn test_exact_matches() {
		let needle = "deadbe";
		let haystack = vec!["deadbe", "deadbeef", "deadbe", "deadbf", "deadbe", "deadbeefg", "deadbe"];

		let matches = match_list(needle, &haystack, &Config::default());

		let exact_matches = matches.iter().filter(|m| m.exact).collect::<Vec<&Match>>();
		assert_eq!(exact_matches.len(), 4);
		for m in &exact_matches {
			assert_eq!(haystack[m.index as usize], needle)
		}
	}

	#[test]
	fn test_prefilter_allows_typo_candidates() {
		let needle = "solf";
		let haystack = vec!["self::", "write"];

		let matches = match_list(
			needle,
			&haystack,
			&Config {
				max_typos: Some(1),
				..Config::default()
			},
		);

		assert_eq!(matches.len(), 1);
		assert_eq!(matches[0].index, 0);
	}

	#[test]
	fn parallel_matches_serial_without_typos() {
		let needle = "deadbe";
		let haystack = vec!["deadbeef", "deadbf", "deadbeefg", "deadbe", "rebuild", "debug", "debut", "dea"];

		let config = Config {
			max_typos: None,
			sort: true,
			..Config::default()
		};
		let serial = match_list(needle, &haystack, &config);
		let parallel = match_list_parallel(needle, &haystack, &config, 4);

		assert_eq!(parallel, serial);
	}

	#[test]
	fn parallel_matches_serial_with_zero_typos() {
		let needle = "tr";
		let haystack = vec!["tracing", "tree", "error", "result", "to", "transport", "trace", "tar"];

		let config = Config {
			max_typos: Some(0),
			sort: true,
			..Config::default()
		};
		let serial = match_list(needle, &haystack, &config);
		let parallel = match_list_parallel(needle, &haystack, &config, 4);

		assert_eq!(parallel, serial);
	}

	#[test]
	fn max_typos_larger_than_needle_len_does_not_underflow() {
		let needle = "a";
		let haystack = vec!["", "a", "bbb"];

		let config = Config {
			max_typos: Some(10),
			..Config::default()
		};
		let matches = match_list(needle, &haystack, &config);

		assert!(!matches.is_empty());
	}

	#[test]
	fn disabling_prefilter_preserves_match_results() {
		let needle = "solf";
		let haystack = vec!["self::", "super::", "write", "solve", "slot", "shelf"];

		let with_prefilter = Config {
			max_typos: Some(1),
			prefilter: true,
			sort: true,
			..Config::default()
		};
		let without_prefilter = Config {
			max_typos: Some(1),
			prefilter: false,
			sort: true,
			..Config::default()
		};

		let with_prefilter_matches = match_list(needle, &haystack, &with_prefilter);
		let without_prefilter_matches = match_list(needle, &haystack, &without_prefilter);

		assert_eq!(with_prefilter_matches, without_prefilter_matches);
	}

	#[test]
	fn greedy_fallback_respects_typo_budget() {
		let needle = "z".repeat(4096);
		let haystack = vec!["a".repeat(5000)];
		let haystack_refs: Vec<&str> = haystack.iter().map(String::as_str).collect();

		let config = Config {
			max_typos: Some(0),
			prefilter: false,
			..Config::default()
		};
		let matches = match_list(&needle, &haystack_refs, &config);

		assert!(matches.is_empty());
	}

	#[test]
	fn matrix_budget_fallback_respects_typo_budget() {
		let needle = "z".repeat(128);
		let haystack = vec!["a".repeat(100)];
		let haystack_refs: Vec<&str> = haystack.iter().map(String::as_str).collect();

		let config = Config {
			max_typos: Some(0),
			prefilter: false,
			..Config::default()
		};
		let matches = match_list(&needle, &haystack_refs, &config);

		assert!(matches.is_empty());
	}
}
