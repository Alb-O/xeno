use super::match_list;
use crate::one_shot::matcher::match_list_impl;
use crate::{Config, Match};

/// Computes the Smith-Waterman score with affine gaps for a needle against a list of haystacks
/// with multithreading.
///
/// You should call this function with as many haystacks as you have available as it will
/// automatically chunk the haystacks based on string length to avoid unnecessary computation
/// due to SIMD
pub fn match_list_parallel<S1: AsRef<str>, S2: AsRef<str> + Sync + Send>(needle: S1, haystacks: &[S2], config: &Config, max_threads: usize) -> Vec<Match> {
	let max_threads = max_threads.max(1);
	let thread_count = choose_thread_count(haystacks.len(), config.max_typos).clamp(1, max_threads);
	if thread_count == 1 {
		return match_list(needle, haystacks, config);
	}

	let needle = needle.as_ref();
	let items_per_thread = haystacks.len().div_ceil(thread_count);
	let mut matches = if config.max_typos.is_none() {
		Vec::with_capacity(haystacks.len())
	} else {
		Vec::new()
	};

	std::thread::scope(|s| {
		let mut tasks = Vec::new();

		for (thread_idx, haystacks) in haystacks.chunks(items_per_thread).enumerate() {
			let index_offset = (thread_idx * items_per_thread) as u32;
			tasks.push(s.spawn(move || {
				let mut local_matches = if config.max_typos.is_none() {
					Vec::with_capacity(haystacks.len())
				} else {
					Vec::new()
				};
				match_list_impl(needle, haystacks, index_offset, config, &mut local_matches);
				local_matches
			}));
		}

		for task in tasks {
			matches.extend(task.join().expect("parallel match worker panicked"));
		}
	});

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

fn choose_thread_count(haystacks_len: usize, max_typos: Option<u16>) -> usize {
	// TODO: ideally, we'd change this based on the average length of items in the haystack and the
	// length of the needle. Perhaps random sampling would work well?
	let min_items_per_thread = match max_typos {
		Some(0) => 5000,
		// Slower prefilter
		Some(1) => 4000,
		Some(_) => 3000,
		None => 2500,
	};

	haystacks_len / min_items_per_thread
}
