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
mod tests;
