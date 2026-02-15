use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use multiversion as _;
#[cfg(feature = "parallel_sort")]
use rayon as _;
#[cfg(feature = "serde")]
use serde as _;
use xeno_matcher::prefilter::Prefilter;
use xeno_matcher::{Config, IncrementalMatcher, match_list, match_list_parallel};

const NEEDLES: [&str; 3] = ["foo", "deadbeef", "serialfmt"];

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

fn shuffle(rng: &mut XorShift64, values: &mut [u8]) {
	for idx in (1..values.len()).rev() {
		let swap_idx = rng.next_usize(idx + 1);
		values.swap(idx, swap_idx);
	}
}

fn gen_ascii_bytes(rng: &mut XorShift64, len: usize, alphabet: &[u8]) -> Vec<u8> {
	let mut out = Vec::with_capacity(len);
	for _ in 0..len {
		out.push(alphabet[rng.next_usize(alphabet.len())]);
	}
	out
}

fn generate_haystacks(count: usize) -> Vec<String> {
	let mut rng = XorShift64::new(0xA3C5_9F2D_11E4_7B19);
	let lengths = [8usize, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512];
	let cold_alphabet = b"qwxzvkjyupnghtrm";
	let warm_alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789_-/";

	let mut haystacks = Vec::with_capacity(count);
	for _ in 0..count {
		let len = lengths[rng.next_usize(lengths.len())];
		let roll = rng.next_usize(100);
		let needle = NEEDLES[rng.next_usize(NEEDLES.len())].as_bytes();

		let haystack = if roll < 90 {
			String::from_utf8(gen_ascii_bytes(&mut rng, len, cold_alphabet)).expect("cold haystack is valid ASCII")
		} else if roll < 95 {
			let mut out = gen_ascii_bytes(&mut rng, len, warm_alphabet);
			let mut scrambled = needle.to_vec();
			shuffle(&mut rng, &mut scrambled);
			for &ch in &scrambled {
				let idx = rng.next_usize(len);
				out[idx] = ch;
			}
			String::from_utf8(out).expect("unordered haystack is valid ASCII")
		} else {
			let mut out = gen_ascii_bytes(&mut rng, len, warm_alphabet);
			if !needle.is_empty() {
				if len >= needle.len() {
					let start = rng.next_usize(len - needle.len());
					out[start..(start + needle.len())].copy_from_slice(needle);
					if start > 0 {
						out[start - 1] = b'_';
					}
					if start + needle.len() < len {
						out[start + needle.len()] = b'-';
					}
				} else {
					out.copy_from_slice(&needle[..len]);
				}
			}
			String::from_utf8(out).expect("ordered haystack is valid ASCII")
		};

		haystacks.push(haystack);
	}

	haystacks
}

fn match_config(max_typos: Option<u16>) -> Config {
	Config {
		max_typos,
		sort: true,
		..Config::default()
	}
}

fn bench_match_list(c: &mut Criterion) {
	let haystacks = generate_haystacks(10_000);
	let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();

	let mut group = c.benchmark_group("match_list");
	for &needle in &NEEDLES {
		for &max_typos in &[None, Some(0), Some(1)] {
			let config = match_config(max_typos);
			let label = format!("needle={needle}:max_typos={max_typos:?}");
			group.bench_with_input(BenchmarkId::new("serial", label), &config, |b, config| {
				b.iter(|| black_box(match_list(needle, &haystack_refs, config)));
			});
		}
	}
	group.finish();
}

fn bench_parallel_vs_serial(c: &mut Criterion) {
	let haystacks = generate_haystacks(10_000);
	let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();
	let needle = "deadbeef";

	let mut group = c.benchmark_group("parallel_vs_serial");
	for &max_typos in &[None, Some(0), Some(1)] {
		let config = match_config(max_typos);
		let label = format!("max_typos={max_typos:?}");
		group.bench_with_input(BenchmarkId::new("serial", label.clone()), &config, |b, config| {
			b.iter(|| black_box(match_list(needle, &haystack_refs, config)));
		});
		group.bench_with_input(BenchmarkId::new("parallel_8", label), &config, |b, config| {
			b.iter(|| black_box(match_list_parallel(needle, &haystack_refs, config, 8)));
		});
	}
	group.finish();
}

fn bench_incremental_typing(c: &mut Criterion) {
	let haystacks = generate_haystacks(10_000);
	let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();
	let sequence = ["", "d", "de", "dea", "dead", "deadb", "deadbe", "deadbee", "deadbeef"];
	let config = match_config(Some(1));

	c.bench_function("incremental_typing", |b| {
		b.iter_batched(
			|| IncrementalMatcher::new(&haystack_refs),
			|mut matcher| {
				for needle in &sequence {
					black_box(matcher.match_needle(needle, &config));
				}
			},
			BatchSize::SmallInput,
		);
	});
}

fn bench_prefilter(c: &mut Criterion) {
	let haystacks = generate_haystacks(10_000);
	let needle = "deadbeef";

	let mut group = c.benchmark_group("prefilter_scan");
	group.bench_function("unordered_insensitive_max0", |b| {
		b.iter_batched(
			|| Prefilter::new(needle, 0),
			|prefilter| {
				let mut matches = 0usize;
				for haystack in &haystacks {
					if prefilter.match_haystack_unordered_insensitive(haystack.as_bytes()) {
						matches += 1;
					}
				}
				black_box(matches);
			},
			BatchSize::SmallInput,
		);
	});

	group.bench_function("unordered_typos_insensitive_max1", |b| {
		b.iter_batched(
			|| Prefilter::new(needle, 1),
			|prefilter| {
				let mut matches = 0usize;
				for haystack in &haystacks {
					if prefilter.match_haystack_unordered_typos_insensitive(haystack.as_bytes()) {
						matches += 1;
					}
				}
				black_box(matches);
			},
			BatchSize::SmallInput,
		);
	});
	group.finish();
}

criterion_group!(benches, bench_match_list, bench_parallel_vs_serial, bench_incremental_typing, bench_prefilter,);
criterion_main!(benches);
