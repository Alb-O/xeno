use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use multiversion as _;
#[cfg(feature = "parallel_sort")]
use rayon as _;
#[cfg(feature = "serde")]
use serde as _;
#[cfg(feature = "incremental")]
use xeno_matcher::IncrementalMatcher;
use xeno_matcher::prefilter::Prefilter;
use xeno_matcher::smith_waterman::simd::{smith_waterman_scores, smith_waterman_scores_typos};
use xeno_matcher::{Config, Scoring, match_list, match_list_parallel};

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

#[cfg(feature = "incremental")]
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

fn as_str_array<const L: usize>(xs: &[String; L]) -> [&str; L] {
	std::array::from_fn(|i| xs[i].as_str())
}

fn ordered_haystack_fixed_len(len: usize, needle: &str, seed: u64) -> String {
	let mut rng = XorShift64::new(seed);
	let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
	let mut out = gen_ascii_bytes(&mut rng, len, alphabet);
	let needle_b = needle.as_bytes();
	if !needle_b.is_empty() && len > 0 {
		let start = if len > needle_b.len() { rng.next_usize(len - needle_b.len() + 1) } else { 0 };
		let copy_len = needle_b.len().min(len - start);
		out[start..start + copy_len].copy_from_slice(&needle_b[..copy_len]);
		if start > 0 {
			out[start - 1] = b'_';
		}
		if start + copy_len < len {
			out[start + copy_len] = b'-';
		}
	}
	String::from_utf8(out).expect("ordered haystack valid ASCII")
}

fn tie_heavy_haystack_fixed_len(len: usize, needle: &str, seed: u64) -> String {
	let mut rng = XorShift64::new(seed);
	let nb = needle.as_bytes();
	let first = nb.first().copied().unwrap_or(b'a');
	let last = nb.last().copied().unwrap_or(b'a');
	let fillers = [b'_', b'-', b'/', b'x'];
	let mut out = Vec::with_capacity(len);
	if len == 0 {
		return String::new();
	}
	out.push(first);
	let fill_limit = len.saturating_sub(5);
	while out.len() < fill_limit {
		out.push(fillers[rng.next_usize(fillers.len())]);
	}
	if out.len() < len {
		out.push(last);
	}
	while out.len() < len {
		out.push(last);
	}
	String::from_utf8(out).expect("tie-heavy haystack valid ASCII")
}

fn ordered_haystack_two_needles(len: usize, needle_a: &str, needle_b: &str, seed: u64) -> String {
	let mut bytes = ordered_haystack_fixed_len(len, needle_a, seed).into_bytes();
	let nb = needle_b.as_bytes();
	if !nb.is_empty() && len >= nb.len() + 2 {
		let start = (len / 2).min(len - nb.len());
		bytes[start..start + nb.len()].copy_from_slice(nb);
		if start > 0 {
			bytes[start - 1] = b'_';
		}
		if start + nb.len() < len {
			bytes[start + nb.len()] = b'-';
		}
	}
	String::from_utf8(bytes).expect("two-needle haystack valid ASCII")
}

fn bench_sw_micro(c: &mut Criterion) {
	let scoring = Scoring::default();
	let needle = "deadbeef";
	let mut group = c.benchmark_group("sw_micro");

	{
		let ordered: [String; 8] = std::array::from_fn(|i| ordered_haystack_fixed_len(64, needle, 0x1111_0000 + i as u64));
		let tie: [String; 8] = std::array::from_fn(|i| tie_heavy_haystack_fixed_len(64, needle, 0x2222_0000 + i as u64));
		let ordered_refs = as_str_array(&ordered);
		let tie_refs = as_str_array(&tie);
		group.bench_function(BenchmarkId::new("scores_only", "W64:ordered"), |b| {
			b.iter(|| black_box(smith_waterman_scores::<64, 8>(needle, &ordered_refs, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k1", "W64:ordered"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<64, 8>(needle, &ordered_refs, 1, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k1", "W64:tie"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<64, 8>(needle, &tie_refs, 1, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k2", "W64:ordered"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<64, 8>(needle, &ordered_refs, 2, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k2", "W64:tie"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<64, 8>(needle, &tie_refs, 2, &scoring)));
		});
	}

	{
		let ordered: [String; 8] = std::array::from_fn(|i| ordered_haystack_fixed_len(256, needle, 0x3333_0000 + i as u64));
		let tie: [String; 8] = std::array::from_fn(|i| tie_heavy_haystack_fixed_len(256, needle, 0x4444_0000 + i as u64));
		let ordered_refs = as_str_array(&ordered);
		let tie_refs = as_str_array(&tie);
		group.bench_function(BenchmarkId::new("scores_only", "W256:ordered"), |b| {
			b.iter(|| black_box(smith_waterman_scores::<256, 8>(needle, &ordered_refs, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k1", "W256:ordered"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<256, 8>(needle, &ordered_refs, 1, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k1", "W256:tie"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<256, 8>(needle, &tie_refs, 1, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k2", "W256:ordered"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<256, 8>(needle, &ordered_refs, 2, &scoring)));
		});
		group.bench_function(BenchmarkId::new("typos_k2", "W256:tie"), |b| {
			b.iter(|| black_box(smith_waterman_scores_typos::<256, 8>(needle, &tie_refs, 2, &scoring)));
		});
	}

	group.finish();
}

fn bench_match_list_typo_guardrails(c: &mut Criterion) {
	let needle_sw = "deadbeef";
	let needle_greedy = "serialfmt";
	let len = 256usize;
	let haystacks: Vec<String> = (0..2_000)
		.map(|i| ordered_haystack_two_needles(len, needle_sw, needle_greedy, 0xABC0_0000 + i as u64))
		.collect();
	let hay_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();
	let cfg = Config {
		prefilter: false,
		max_typos: Some(1),
		sort: false,
		scoring: Scoring::default(),
	};

	let mut group = c.benchmark_group("match_list_typo_guardrails");
	group.bench_with_input(BenchmarkId::new("typo_mode", "W256:SW_path:deadbeef"), &cfg, |b, cfg| {
		b.iter(|| black_box(match_list(needle_sw, &hay_refs, cfg)));
	});
	group.bench_with_input(BenchmarkId::new("typo_mode", "W256:greedy_fallback:serialfmt"), &cfg, |b, cfg| {
		b.iter(|| black_box(match_list(needle_greedy, &hay_refs, cfg)));
	});
	group.finish();
}

#[cfg(feature = "incremental")]
criterion_group!(
	benches,
	bench_match_list,
	bench_parallel_vs_serial,
	bench_incremental_typing,
	bench_prefilter,
	bench_sw_micro,
	bench_match_list_typo_guardrails,
);
#[cfg(not(feature = "incremental"))]
criterion_group!(
	benches,
	bench_match_list,
	bench_parallel_vs_serial,
	bench_prefilter,
	bench_sw_micro,
	bench_match_list_typo_guardrails,
);
criterion_main!(benches);
