use super::bucket::IncrementalBucketTrait;
use super::bucket_collection::IncrementalBucketCollection;
use crate::{Config, Match};

pub struct IncrementalMatcher {
	needle: Option<String>,
	num_haystacks: usize,
	buckets: Vec<Box<dyn IncrementalBucketTrait>>,
	overflow_haystacks: Vec<(u32, String)>,
}

impl IncrementalMatcher {
	pub fn new<S: AsRef<str>>(haystacks: &[S]) -> Self {
		// group haystacks into buckets by length

		let mut buckets: Vec<Box<dyn IncrementalBucketTrait>> = vec![];
		let mut overflow_haystacks = Vec::new();

		let mut collection_size_4 = IncrementalBucketCollection::<'_, 4, 8>::new();
		let mut collection_size_8 = IncrementalBucketCollection::<'_, 8, 8>::new();
		let mut collection_size_12 = IncrementalBucketCollection::<'_, 12, 8>::new();
		let mut collection_size_16 = IncrementalBucketCollection::<'_, 16, 8>::new();
		let mut collection_size_20 = IncrementalBucketCollection::<'_, 20, 8>::new();
		let mut collection_size_24 = IncrementalBucketCollection::<'_, 24, 8>::new();
		let mut collection_size_32 = IncrementalBucketCollection::<'_, 32, 8>::new();
		let mut collection_size_48 = IncrementalBucketCollection::<'_, 48, 8>::new();
		let mut collection_size_64 = IncrementalBucketCollection::<'_, 64, 8>::new();
		let mut collection_size_96 = IncrementalBucketCollection::<'_, 96, 8>::new();
		let mut collection_size_128 = IncrementalBucketCollection::<'_, 128, 8>::new();
		let mut collection_size_160 = IncrementalBucketCollection::<'_, 160, 8>::new();
		let mut collection_size_192 = IncrementalBucketCollection::<'_, 192, 8>::new();
		let mut collection_size_224 = IncrementalBucketCollection::<'_, 224, 8>::new();
		let mut collection_size_256 = IncrementalBucketCollection::<'_, 256, 8>::new();
		let mut collection_size_384 = IncrementalBucketCollection::<'_, 384, 8>::new();
		let mut collection_size_512 = IncrementalBucketCollection::<'_, 512, 8>::new();

		for (i, haystack) in haystacks.iter().enumerate() {
			let i = i as u32;
			let haystack = haystack.as_ref();
			match haystack.len() {
				0..=4 => collection_size_4.add_haystack(haystack, i, &mut buckets),
				5..=8 => collection_size_8.add_haystack(haystack, i, &mut buckets),
				9..=12 => collection_size_12.add_haystack(haystack, i, &mut buckets),
				13..=16 => collection_size_16.add_haystack(haystack, i, &mut buckets),
				17..=20 => collection_size_20.add_haystack(haystack, i, &mut buckets),
				21..=24 => collection_size_24.add_haystack(haystack, i, &mut buckets),
				25..=32 => collection_size_32.add_haystack(haystack, i, &mut buckets),
				33..=48 => collection_size_48.add_haystack(haystack, i, &mut buckets),
				49..=64 => collection_size_64.add_haystack(haystack, i, &mut buckets),
				65..=96 => collection_size_96.add_haystack(haystack, i, &mut buckets),
				97..=128 => collection_size_128.add_haystack(haystack, i, &mut buckets),
				129..=160 => collection_size_160.add_haystack(haystack, i, &mut buckets),
				161..=192 => collection_size_192.add_haystack(haystack, i, &mut buckets),
				193..=224 => collection_size_224.add_haystack(haystack, i, &mut buckets),
				225..=256 => collection_size_256.add_haystack(haystack, i, &mut buckets),
				257..=384 => collection_size_384.add_haystack(haystack, i, &mut buckets),
				385..=512 => collection_size_512.add_haystack(haystack, i, &mut buckets),
				_ => overflow_haystacks.push((i, haystack.to_owned())),
			};
		}

		collection_size_4.finalize(&mut buckets);
		collection_size_8.finalize(&mut buckets);
		collection_size_12.finalize(&mut buckets);
		collection_size_16.finalize(&mut buckets);
		collection_size_20.finalize(&mut buckets);
		collection_size_24.finalize(&mut buckets);
		collection_size_32.finalize(&mut buckets);
		collection_size_48.finalize(&mut buckets);
		collection_size_64.finalize(&mut buckets);
		collection_size_96.finalize(&mut buckets);
		collection_size_128.finalize(&mut buckets);
		collection_size_160.finalize(&mut buckets);
		collection_size_192.finalize(&mut buckets);
		collection_size_224.finalize(&mut buckets);
		collection_size_256.finalize(&mut buckets);
		collection_size_384.finalize(&mut buckets);
		collection_size_512.finalize(&mut buckets);

		Self {
			needle: None,
			num_haystacks: haystacks.len(),
			buckets,
			overflow_haystacks,
		}
	}

	pub fn match_needle<S: AsRef<str>>(&mut self, needle: S, config: &Config) -> Vec<Match> {
		let needle = needle.as_ref();
		if needle.is_empty() {
			self.needle = Some(String::new());
			return (0..self.num_haystacks)
				.map(|idx| Match {
					index: idx as u32,
					score: 0,
					exact: false,
				})
				.collect();
		}

		let common_prefix_len = self
			.needle
			.as_ref()
			.map(|prev_needle| needle.as_bytes().iter().zip(prev_needle.as_bytes()).take_while(|&(&a, &b)| a == b).count())
			.unwrap_or(0);

		let mut matches = Vec::with_capacity(self.num_haystacks);

		self.process(common_prefix_len, needle, &mut matches, config);
		self.process_overflow(needle, &mut matches, config);
		self.needle = Some(needle.to_owned());

		if config.sort {
			matches.sort_unstable();
		}

		matches
	}

	fn process(&mut self, prefix_to_keep: usize, needle: &str, matches: &mut Vec<Match>, config: &Config) {
		let needle = &needle.as_bytes()[prefix_to_keep..];

		for bucket in self.buckets.iter_mut() {
			bucket.process(prefix_to_keep, needle, matches, config.max_typos, &config.scoring);
		}
	}

	fn process_overflow(&self, needle: &str, matches: &mut Vec<Match>, config: &Config) {
		if self.overflow_haystacks.is_empty() {
			return;
		}

		let overflow_haystack_refs: Vec<&str> = self.overflow_haystacks.iter().map(|(_, haystack)| haystack.as_str()).collect();
		let mut overflow_matches = crate::match_list(needle, &overflow_haystack_refs, config);

		for mtch in &mut overflow_matches {
			mtch.index = self.overflow_haystacks[mtch.index as usize].0;
		}

		matches.extend(overflow_matches);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::r#const::*;

	const CHAR_SCORE: u16 = MATCH_SCORE + MATCHING_CASE_BONUS;

	fn get_score(needle: &str, haystack: &str) -> u16 {
		let mut matcher = IncrementalMatcher::new(&[haystack]);
		matcher.match_needle(needle, &Config::default())[0].score
	}

	#[test]
	fn test_score_basic() {
		assert_eq!(get_score("b", "abc"), CHAR_SCORE);
		assert_eq!(get_score("c", "abc"), CHAR_SCORE);
	}

	#[test]
	fn test_score_prefix() {
		assert_eq!(get_score("a", "abc"), CHAR_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("a", "aabc"), CHAR_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("a", "babc"), CHAR_SCORE);
	}

	#[test]
	fn test_score_offset_prefix() {
		// Give prefix bonus on second char if the first char isn't a letter
		assert_eq!(get_score("a", "-a"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
		assert_eq!(get_score("-a", "-ab"), 2 * CHAR_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("a", "'a"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
		assert_eq!(get_score("a", "Ba"), CHAR_SCORE);
	}

	#[test]
	#[ignore = "Incremental matcher doesn't support exact matches until we implement them in SIMD"]
	fn test_score_exact_match() {
		assert_eq!(get_score("a", "a"), CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
		assert_eq!(get_score("abc", "abc"), 3 * CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
		assert_eq!(get_score("ab", "abc"), 2 * CHAR_SCORE + PREFIX_BONUS);
		// assert_eq!(run_single("abc", "ab"), 2 * CHAR_SCORE + PREFIX_BONUS);
	}

	#[test]
	fn test_score_delimiter() {
		assert_eq!(get_score("b", "a-b"), CHAR_SCORE + DELIMITER_BONUS);
		assert_eq!(get_score("a", "a-b-c"), CHAR_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("b", "a--b"), CHAR_SCORE + DELIMITER_BONUS);
		assert_eq!(get_score("c", "a--bc"), CHAR_SCORE);
		assert_eq!(get_score("a", "-a--bc"), CHAR_SCORE + OFFSET_PREFIX_BONUS);
	}

	#[test]
	fn test_score_no_delimiter_for_delimiter_chars() {
		assert_eq!(get_score("-", "a-bc"), CHAR_SCORE);
		assert_eq!(get_score("-", "a--bc"), CHAR_SCORE);
		assert!(get_score("a_b", "a_bb") > get_score("a_b", "a__b"));
	}

	#[test]
	fn test_score_affine_gap() {
		assert_eq!(get_score("test", "Uterst"), CHAR_SCORE * 4 - GAP_OPEN_PENALTY);
		assert_eq!(get_score("test", "Uterrst"), CHAR_SCORE * 4 - GAP_OPEN_PENALTY - GAP_EXTEND_PENALTY);
	}

	#[test]
	fn test_score_capital_bonus() {
		assert_eq!(get_score("a", "A"), MATCH_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("A", "Aa"), CHAR_SCORE + PREFIX_BONUS);
		assert_eq!(get_score("D", "forDist"), CHAR_SCORE + CAPITALIZATION_BONUS);
		assert_eq!(get_score("D", "foRDist"), CHAR_SCORE);
	}

	#[test]
	fn test_score_prefix_beats_delimiter() {
		assert!(get_score("swap", "swap(test)") > get_score("swap", "iter_swap(test)"),);
	}

	#[test]
	fn test_empty_needle_returns_all_haystacks() {
		let mut matcher = IncrementalMatcher::new(&["abc", "def", "ghi"]);
		let matches = matcher.match_needle("", &Config::default());

		assert_eq!(matches.len(), 3);
		for (idx, mtch) in matches.iter().enumerate() {
			assert_eq!(mtch.index, idx as u32);
			assert_eq!(mtch.score, 0);
			assert!(!mtch.exact);
		}
	}

	#[test]
	fn test_haystacks_larger_than_simd_limit_are_included() {
		let long = "a".repeat(700);
		let mut matcher = IncrementalMatcher::new(&[long]);
		let matches = matcher.match_needle("a", &Config::default());

		assert_eq!(matches.len(), 1);
		assert_eq!(matches[0].index, 0);
		assert!(matches[0].score > 0);
	}
}
