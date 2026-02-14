use super::bucket::IncrementalBucket;
use super::bucket_collection::IncrementalBucketCollection;
use crate::engine::CandidateFilter;
use crate::limits::exceeds_typo_budget;
use crate::smith_waterman::greedy::match_greedy;
use crate::{Config, Match, Scoring};

macro_rules! define_incremental_buckets {
	($(($name:ident, $width:literal, $range:pat)),* $(,)?) => {
		struct IncrementalBuckets<'a> {
			$($name: Vec<IncrementalBucket<'a, $width, 8>>,)*
		}

		impl<'a> IncrementalBuckets<'a> {
			fn new() -> Self {
				Self {
					$($name: Vec::new(),)*
				}
			}

			fn process_all(
				&mut self,
				prefix_to_keep: usize,
				needle: &str,
				matches: &mut Vec<Match>,
				max_typos: Option<u16>,
				scoring: &Scoring,
			) {
				$(
					for bucket in &mut self.$name {
						bucket.process(prefix_to_keep, needle, matches, max_typos, scoring);
					}
				)*
			}
		}
	};
}
crate::for_each_bucket_spec!(define_incremental_buckets);

pub struct IncrementalMatcher<'a> {
	needle: Option<String>,
	num_haystacks: usize,
	buckets: IncrementalBuckets<'a>,
	overflow_haystacks: Vec<(u32, &'a str)>,
}

impl<'a> IncrementalMatcher<'a> {
	pub fn new<S: AsRef<str>>(haystacks: &'a [S]) -> Self {
		// group haystacks into buckets by length

		let mut buckets = IncrementalBuckets::new();
		let mut overflow_haystacks = Vec::new();

		macro_rules! build_collections {
			($(($name:ident, $width:literal, $range:pat)),* $(,)?) => {
				$(let mut $name = IncrementalBucketCollection::<'a, $width, 8>::new();)*

				for (i, haystack) in haystacks.iter().enumerate() {
					let i = i as u32;
					let haystack = haystack.as_ref();
					match haystack.len() {
						$($range => $name.add_haystack(haystack, i, &mut buckets.$name),)*
						_ => overflow_haystacks.push((i, haystack)),
					};
				}

				$($name.finalize(&mut buckets.$name);)*
			};
		}
		crate::for_each_bucket_spec!(build_collections);

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
		let prefix_to_keep = if config.max_typos.is_some() { 0 } else { prefix_to_keep };

		self.buckets.process_all(prefix_to_keep, needle, matches, config.max_typos, &config.scoring);
	}

	fn process_overflow(&self, needle: &str, matches: &mut Vec<Match>, config: &Config) {
		if self.overflow_haystacks.is_empty() {
			return;
		}

		let filter = CandidateFilter::new(needle, config);

		for &(index, haystack) in &self.overflow_haystacks {
			if !filter.allows(haystack) {
				continue;
			}

			let (score, indices, exact) = match_greedy(needle, haystack, &config.scoring);
			if exceeds_typo_budget(config.max_typos, needle, indices.len()) {
				continue;
			}

			matches.push(Match { index, score, exact });
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::r#const::*;

	const CHAR_SCORE: u16 = MATCH_SCORE + MATCHING_CASE_BONUS;

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

	fn gen_ascii_bytes(rng: &mut XorShift64, len: usize, alphabet: &[u8]) -> Vec<u8> {
		let mut out = Vec::with_capacity(len);
		for _ in 0..len {
			out.push(alphabet[rng.next_usize(alphabet.len())]);
		}
		out
	}

	fn generate_haystacks(count: usize, lengths: &[usize], needle: &str) -> Vec<String> {
		let mut rng = XorShift64::new(0x61CF_2A94_D5A8_9E31);
		let cold_alphabet = b"qwxzvkjyupnghtrm";
		let warm_alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789_-/";
		let mut haystacks = Vec::with_capacity(count);

		for _ in 0..count {
			let len = lengths[rng.next_usize(lengths.len())];
			let roll = rng.next_usize(100);
			let haystack = if roll < 90 {
				String::from_utf8(gen_ascii_bytes(&mut rng, len, cold_alphabet)).expect("cold haystack is valid ASCII")
			} else if roll < 95 {
				let mut out = gen_ascii_bytes(&mut rng, len, warm_alphabet);
				for &ch in needle.as_bytes() {
					out[rng.next_usize(len)] = ch;
				}
				String::from_utf8(out).expect("unordered haystack is valid ASCII")
			} else {
				let mut out = gen_ascii_bytes(&mut rng, len, warm_alphabet);
				if len >= needle.len() {
					let start = rng.next_usize(len - needle.len() + 1);
					out[start..(start + needle.len())].copy_from_slice(needle.as_bytes());
				} else {
					out.copy_from_slice(&needle.as_bytes()[..len]);
				}
				String::from_utf8(out).expect("ordered haystack is valid ASCII")
			};

			haystacks.push(haystack);
		}

		haystacks
	}

	fn get_score(needle: &str, haystack: &str) -> u16 {
		let haystacks = [haystack];
		let mut matcher = IncrementalMatcher::new(&haystacks);
		let config = Config {
			max_typos: None,
			prefilter: false,
			..Config::default()
		};
		matcher.match_needle(needle, &config)[0].score
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
	fn test_score_exact_match() {
		assert_eq!(get_score("a", "a"), CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
		assert_eq!(get_score("abc", "abc"), 3 * CHAR_SCORE + EXACT_MATCH_BONUS + PREFIX_BONUS);
		assert_eq!(get_score("ab", "abc"), 2 * CHAR_SCORE + PREFIX_BONUS);
		// assert_eq!(run_single("abc", "ab"), 2 * CHAR_SCORE + PREFIX_BONUS);
	}

	#[test]
	fn test_exact_bonus_delta_matches_constant() {
		let needle = "deadbeef";
		let haystacks = [needle];
		let mut matcher_with_bonus = IncrementalMatcher::new(&haystacks);
		let mut matcher_without_bonus = IncrementalMatcher::new(&haystacks);

		let with_bonus = Config {
			max_typos: None,
			prefilter: false,
			..Config::default()
		};
		let mut without_bonus_scoring = with_bonus.scoring.clone();
		without_bonus_scoring.exact_match_bonus = 0;
		let without_bonus = Config {
			max_typos: None,
			prefilter: false,
			scoring: without_bonus_scoring,
			..Config::default()
		};

		let with_bonus_match = matcher_with_bonus.match_needle(needle, &with_bonus);
		let without_bonus_match = matcher_without_bonus.match_needle(needle, &without_bonus);

		assert_eq!(with_bonus_match.len(), 1);
		assert_eq!(without_bonus_match.len(), 1);
		assert!(with_bonus_match[0].exact);
		assert!(without_bonus_match[0].exact);
		assert_eq!(with_bonus_match[0].score, without_bonus_match[0].score + EXACT_MATCH_BONUS);
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
		let haystacks = [long];
		let mut matcher = IncrementalMatcher::new(&haystacks);
		let matches = matcher.match_needle("a", &Config::default());

		assert_eq!(matches.len(), 1);
		assert_eq!(matches[0].index, 0);
		assert!(matches[0].score > 0);
	}

	#[test]
	fn test_typing_sequence_and_backspace_does_not_panic() {
		let haystacks = ["deadbeef", "debug", "delta", "deal", "defer", "dog"];
		let mut matcher = IncrementalMatcher::new(&haystacks);
		let config = Config {
			prefilter: false,
			..Config::default()
		};
		let sequence = [
			"d", "de", "dea", "dead", "deadb", "deadbe", "deadbee", "deadbeef", "deadbee", "deadbe", "deadb", "dead", "dea", "de", "d", "",
		];

		for needle in &sequence {
			let matches = matcher.match_needle(needle, &config);
			let expected = crate::match_list(needle, &haystacks, &config);
			assert_eq!(matches, expected, "incremental mismatch for needle '{needle}'");
			if needle.is_empty() {
				assert_eq!(matches.len(), haystacks.len());
			}
		}
	}

	#[test]
	fn parity_with_one_shot_for_typing_sequence() {
		let needle = "deadbeef";
		let lengths = [8usize, 12, 16, 24, 32, 48, 64];
		let haystacks = generate_haystacks(512, &lengths, needle);
		let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();
		let sequence = ["de", "dea", "dead", "deadb", "deadbe", "deadbee", "deadbeef", "deadbee", "deadbe"];

		for max_typos in [None, Some(0), Some(1)] {
			let config = Config {
				max_typos,
				prefilter: false,
				sort: true,
				..Config::default()
			};
			let mut incremental = IncrementalMatcher::new(&haystack_refs);

			for current_needle in &sequence {
				let incremental_matches = incremental.match_needle(current_needle, &config);
				let one_shot_matches = crate::match_list(current_needle, &haystack_refs, &config);
				assert_eq!(
					incremental_matches, one_shot_matches,
					"parity mismatch for needle '{current_needle}', max_typos={max_typos:?}"
				);
			}
		}
	}

	#[test]
	fn parity_with_one_shot_forced_greedy_fallback() {
		let needle = "deadbeef";
		let lengths = [500usize, 512];
		let haystacks = generate_haystacks(256, &lengths, needle);
		let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();
		let sequence = ["deadbeef", "deadbee", "deadbeef"];

		for max_typos in [Some(0), Some(1)] {
			let config = Config {
				max_typos,
				prefilter: false,
				sort: true,
				..Config::default()
			};
			let mut incremental = IncrementalMatcher::new(&haystack_refs);

			for current_needle in &sequence {
				let incremental_matches = incremental.match_needle(current_needle, &config);
				let one_shot_matches = crate::match_list(current_needle, &haystack_refs, &config);
				assert_eq!(
					incremental_matches, one_shot_matches,
					"fallback parity mismatch for needle '{current_needle}', max_typos={max_typos:?}"
				);
			}
		}
	}

	#[test]
	fn parity_with_one_shot_for_overflow_haystacks() {
		let needle = "deadbeef";
		let long_match = format!("{needle}{}", "a".repeat(700));
		let long_non_match = "a".repeat(700);
		let haystacks = ["deadbeef".to_string(), long_match, long_non_match];
		let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();

		for max_typos in [None, Some(0), Some(1)] {
			for prefilter in [false, true] {
				let config = Config {
					max_typos,
					prefilter,
					sort: true,
					..Config::default()
				};

				let mut incremental = IncrementalMatcher::new(&haystack_refs);
				let incremental_matches = incremental.match_needle(needle, &config);
				let one_shot_matches = crate::match_list(needle, &haystack_refs, &config);

				assert_eq!(
					incremental_matches, one_shot_matches,
					"overflow parity mismatch for max_typos={max_typos:?}, prefilter={prefilter}"
				);
			}
		}
	}
}
