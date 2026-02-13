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
mod tests {
	use super::*;
	use crate::r#const::EXACT_MATCH_BONUS;
	use crate::match_list_parallel;

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

	fn generate_haystacks(count: usize, needle: &str) -> Vec<String> {
		let mut rng = XorShift64::new(0x7C13_9A20_F1E2_BD59);
		let lengths = [8usize, 12, 16, 24, 32, 48, 64];
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
				if !needle.is_empty() {
					if len > needle.len() {
						let start = rng.next_usize(len - needle.len());
						out[start..(start + needle.len())].copy_from_slice(needle.as_bytes());
						if start > 0 {
							out[start - 1] = b'_';
						}
						if start + needle.len() < len {
							out[start + needle.len()] = b'-';
						}
					} else {
						out.copy_from_slice(needle.as_bytes());
					}
				}
				String::from_utf8(out).expect("ordered haystack is valid ASCII")
			};

			haystacks.push(haystack);
		}

		haystacks
	}

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
	fn parallel_matches_serial_across_typo_configs() {
		let needle = "deadbeef";
		let haystack = generate_haystacks(10_000, needle);
		let haystack_refs: Vec<&str> = haystack.iter().map(String::as_str).collect();

		for max_typos in [None, Some(0), Some(1)] {
			let config = Config {
				max_typos,
				sort: true,
				..Config::default()
			};

			let serial = match_list(needle, &haystack_refs, &config);
			let parallel = match_list_parallel(needle, &haystack_refs, &config, 8);

			assert_eq!(parallel, serial, "parallel mismatch for max_typos={max_typos:?}");
		}
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
		let haystack = generate_haystacks(2_000, needle);
		let haystack_refs: Vec<&str> = haystack.iter().map(String::as_str).collect();

		for max_typos in [Some(0), Some(1)] {
			let with_prefilter = Config {
				max_typos,
				prefilter: true,
				sort: true,
				..Config::default()
			};
			let without_prefilter = Config {
				max_typos,
				prefilter: false,
				sort: true,
				..Config::default()
			};

			let with_prefilter_matches = match_list(needle, &haystack_refs, &with_prefilter);
			let without_prefilter_matches = match_list(needle, &haystack_refs, &without_prefilter);

			assert_eq!(
				with_prefilter_matches, without_prefilter_matches,
				"prefilter mismatch for max_typos={max_typos:?}"
			);
		}
	}

	#[test]
	fn exact_bonus_delta_matches_constant() {
		let needle = "deadbeef";
		let haystack = [needle];

		let with_bonus = Config {
			max_typos: None,
			prefilter: false,
			..Config::default()
		};
		let mut no_bonus_scoring = with_bonus.scoring.clone();
		no_bonus_scoring.exact_match_bonus = 0;
		let without_bonus = Config {
			max_typos: None,
			prefilter: false,
			scoring: no_bonus_scoring,
			..Config::default()
		};

		let with_bonus_matches = match_list(needle, &haystack, &with_bonus);
		let without_bonus_matches = match_list(needle, &haystack, &without_bonus);

		assert_eq!(with_bonus_matches.len(), 1);
		assert_eq!(without_bonus_matches.len(), 1);
		assert!(with_bonus_matches[0].exact);
		assert!(without_bonus_matches[0].exact);
		assert_eq!(with_bonus_matches[0].score, without_bonus_matches[0].score + EXACT_MATCH_BONUS);
	}

	#[test]
	fn greedy_fallback_respects_typo_budget() {
		let needle = "z".repeat(4096);
		let haystack = ["a".repeat(5000)];
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
		let haystack = ["a".repeat(100)];
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
	fn greedy_fallback_handles_shorter_haystack_without_panic() {
		let needle = "z".repeat(4096);
		let haystack = ["a".repeat(3000)];
		let haystack_refs: Vec<&str> = haystack.iter().map(String::as_str).collect();

		let config = Config {
			max_typos: Some(5000),
			prefilter: false,
			..Config::default()
		};
		let matches = match_list(&needle, &haystack_refs, &config);

		assert_eq!(matches.len(), 1);
		assert_eq!(matches[0].index, 0);
	}
}
