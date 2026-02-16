/// Counts typos for the best-scoring alignment via tie-aware traceback.
///
/// Among all end positions in the last needle row that achieve the maximum score,
/// and among all traceback paths that follow max-predecessor edges, returns the
/// minimum typo count. This eliminates false negatives caused by arbitrary
/// tie-breaking in a fixed diag>left>up priority.
///
/// Uses a flat `Vec<u16>` memo keyed by `(col, row)`. The running score is not
/// part of the memo key because for `row > 0` it always equals `m[col][row]`,
/// and `row == 0` is handled via a closed-form expression.
pub fn typos_from_score_matrix(score_matrix: &[&[u16]]) -> u16 {
	if score_matrix.is_empty() {
		return 0;
	}
	let n = score_matrix.len();
	let h = score_matrix[0].len();
	if h == 0 {
		return n as u16;
	}

	let last = score_matrix.last().unwrap();
	let mut end_score = 0u16;
	for &v in last.iter() {
		end_score = end_score.max(v);
	}
	if end_score == 0 {
		return n as u16;
	}

	let mut memo = vec![u16::MAX; n * h];
	let col = n - 1;
	let mut best = u16::MAX;
	for (row, &s) in last.iter().enumerate() {
		if s != end_score {
			continue;
		}
		best = best.min(min_typos_traceback(score_matrix, col, row, h, &mut memo));
	}
	best
}

/// Recursive min-typo traceback exploring all max-predecessor tie paths.
///
/// Counting rules:
/// * `row == 0` (closed form): `col + if m[col][0] == 0 { 1 } else { 0 }` — forced left
///   moves never branch, so the entire row-0 tail is computed in one shot.
/// * `col == 0`: `if m[0][row] == 0 { 1 } else { 0 }` — no match for needle[0].
/// * `diag` where `diag >= cur` (substitution): +1 typo.
/// * `left` (skip needle char): +1 typo.
/// * `up` (skip haystack char): +0 typos.
///
/// The running score is always `m[col][row]` for `row > 0`, so mismatch detection
/// uses the cell value directly and memo is keyed by `(col, row)` only.
fn min_typos_traceback(m: &[&[u16]], col: usize, row: usize, h: usize, memo: &mut [u16]) -> u16 {
	if row == 0 {
		return col as u16 + if m[col][0] == 0 { 1 } else { 0 };
	}
	if col == 0 {
		return if m[0][row] == 0 { 1 } else { 0 };
	}
	let idx = col * h + row;
	if memo[idx] != u16::MAX {
		return memo[idx];
	}

	let cur = m[col][row];
	let diag = m[col - 1][row - 1];
	let left = m[col - 1][row];
	let up = m[col][row - 1];
	let max_prev = diag.max(left).max(up);
	let mut best = u16::MAX;
	if diag == max_prev {
		let add: u16 = if diag >= cur { 1 } else { 0 };
		best = best.min(add.saturating_add(min_typos_traceback(m, col - 1, row - 1, h, memo)));
	}
	if left == max_prev {
		best = best.min(1u16.saturating_add(min_typos_traceback(m, col - 1, row, h, memo)));
	}
	if up == max_prev {
		best = best.min(min_typos_traceback(m, col, row - 1, h, memo));
	}
	memo[idx] = best;
	best
}

#[cfg(test)]
mod tests {
	use super::super::smith_waterman;
	use super::typos_from_score_matrix;
	use crate::Scoring;

	fn get_typos(needle: &str, haystack: &str) -> u16 {
		let (_, _, score_matrix, _) = smith_waterman(needle, haystack, &Scoring::default());
		let score_matrix_ref = score_matrix.iter().map(|v| v.as_slice()).collect::<Vec<_>>();
		typos_from_score_matrix(&score_matrix_ref)
	}

	#[test]
	fn test_typos_basic() {
		assert_eq!(get_typos("a", "abc"), 0);
		assert_eq!(get_typos("b", "abc"), 0);
		assert_eq!(get_typos("c", "abc"), 0);
		assert_eq!(get_typos("ac", "abc"), 0);

		assert_eq!(get_typos("d", "abc"), 1);
		assert_eq!(get_typos("da", "abc"), 1);
		assert_eq!(get_typos("dc", "abc"), 1);
		assert_eq!(get_typos("ad", "abc"), 1);
		assert_eq!(get_typos("adc", "abc"), 1);
		assert_eq!(get_typos("add", "abc"), 2);
		assert_eq!(get_typos("ddd", "abc"), 3);
		// Empty haystack produces empty score matrix; heuristic traceback not applicable.
	}

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

		fn next_usize(&mut self, upper: usize) -> usize {
			if upper <= 1 {
				return 0;
			}
			(self.next_u64() as usize) % upper
		}
	}

	/// Old deterministic traceback (fixed diag>left>up tie-break) kept for comparison tests.
	fn typos_from_score_matrix_deterministic_old(score_matrix: &[&[u16]]) -> u16 {
		if score_matrix.is_empty() {
			return 0;
		}
		let mut typo_count = 0;
		let mut score = 0;
		let mut positions = 0;
		let last_column = score_matrix.last().unwrap();
		for (idx, &row_score) in last_column.iter().enumerate() {
			if row_score > score {
				score = row_score;
				positions = idx;
			}
		}
		let mut col_idx = score_matrix.len() - 1;
		let mut row_idx: usize = positions;
		while col_idx > 0 {
			if row_idx == 0 {
				typo_count += 1;
				col_idx -= 1;
				continue;
			}
			let diag = score_matrix[col_idx - 1][row_idx - 1];
			let left = score_matrix[col_idx - 1][row_idx];
			let up = score_matrix[col_idx][row_idx - 1];
			if diag >= left && diag >= up {
				if diag >= score {
					typo_count += 1;
				}
				row_idx -= 1;
				col_idx -= 1;
				score = diag;
			} else if left >= up {
				typo_count += 1;
				col_idx -= 1;
				score = left;
			} else {
				row_idx -= 1;
				score = up;
			}
		}
		if col_idx == 0 && score == 0 {
			typo_count += 1;
		}
		typo_count
	}

	/// Returns `(end_score, new_tie_aware_typos, old_deterministic_typos)`.
	fn analyze_contract(needle: &str, haystack: &str, scoring: &Scoring) -> Option<(u16, u16, u16)> {
		if needle.is_empty() || haystack.is_empty() {
			return None;
		}
		let (_score, _typos, score_matrix, _exact) = smith_waterman(needle, haystack, scoring);
		if score_matrix.is_empty() || score_matrix[0].is_empty() {
			return None;
		}
		let last_row = score_matrix.last().unwrap();
		let end_score = *last_row.iter().max().unwrap_or(&0);
		if end_score == 0 {
			return None;
		}
		let refs = score_matrix.iter().map(|v| v.as_slice()).collect::<Vec<_>>();
		let new_typos = typos_from_score_matrix(&refs);
		let old_typos = typos_from_score_matrix_deterministic_old(&refs);
		Some((end_score, new_typos, old_typos))
	}

	/// Proves the tie-aware traceback is strictly better than the old deterministic one.
	///
		/// Finds a case where `old_det > new_min` on the same score matrix.
	#[test]
	fn tie_aware_traceback_improves_over_deterministic() {
		let scoring = Scoring::default();
		let mut found: Option<(String, String, u16, u16, u16)> = None;

		let needles = ["ab", "abc", "abcd", "aB", "Ab", "Aa", "a/b", "ab-c"];
		let fills = ['_', '-', '/', 'x'];
		let gaps = [0usize, 1, 2, 3, 4, 8, 16, 32, 64, 96, 128, 192, 256, 384, 512];

		'outer: for &needle in &needles {
			if needle.len() > 4 {
				continue;
			}
			let first = needle.as_bytes()[0] as char;
			let last = needle.as_bytes()[needle.len() - 1] as char;
			for &fill in &fills {
				for &gap in &gaps {
					let mut haystack = String::new();
					haystack.push(first);
					haystack.extend(std::iter::repeat_n(fill, gap));
					haystack.push(last);
					haystack.extend(std::iter::repeat_n(last, 4));

					if let Some((end_score, new_typos, old_typos)) = analyze_contract(needle, &haystack, &scoring)
						&& old_typos > new_typos {
							found = Some((needle.to_string(), haystack, end_score, new_typos, old_typos));
							break 'outer;
						}
				}
			}
		}

		if found.is_none() {
			let mut rng = XorShift64::new(0x9E37_79B9_7F4A_7C15);
			let needle_alpha = b"abAB/_-";
			let hay_alpha = b"abAB/_-xx--__//";
			for _ in 0..25_000 {
				let nlen = 2 + rng.next_usize(3);
				let hlen = 8 + rng.next_usize(120);
				let mut needle = String::with_capacity(nlen);
				for _ in 0..nlen {
					needle.push(needle_alpha[rng.next_usize(needle_alpha.len())] as char);
				}
				let mut hay = String::with_capacity(hlen);
				for _ in 0..hlen {
					hay.push(hay_alpha[rng.next_usize(hay_alpha.len())] as char);
				}
				if let Some((end_score, new_typos, old_typos)) = analyze_contract(&needle, &hay, &scoring)
					&& old_typos > new_typos {
						found = Some((needle, hay, end_score, new_typos, old_typos));
						break;
					}
			}
		}

		let (_needle, _haystack, end_score, new_typos, old_typos) =
			found.expect("no witness found for old_det > new_min; traceback improvement may need broader search");

		assert!(end_score > 0);
		assert!(old_typos > new_typos, "old={old_typos} should exceed new={new_typos}");
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
		let needles = ["foo", "deadbeef", "serialfmt"];
		let mut haystacks = Vec::with_capacity(count);
		for _ in 0..count {
			let len = lengths[rng.next_usize(lengths.len())];
			let roll = rng.next_usize(100);
			let needle = needles[rng.next_usize(needles.len())].as_bytes();
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

	/// Stats harness comparing old deterministic traceback vs new tie-aware traceback.
	///
	/// Run:
	/// `cargo test -p xeno-matcher typo_contract_tie_break_stats -- --ignored --nocapture`
	#[test]
	#[ignore]
	fn typo_contract_tie_break_stats() {
		let scoring = Scoring::default();
		let needles = ["foo", "deadbeef", "serialfmt"];
		let budgets: [u16; 3] = [0, 1, 2];
		let haystacks = generate_haystacks(2_000);

		let mut total_scored: u64 = 0;
		let mut old_accept = vec![0u64; budgets.len()];
		let mut old_reject = vec![0u64; budgets.len()];
		let mut new_recovered = vec![0u64; budgets.len()];
		let mut max_delta: u16 = 0;

		for needle in needles {
			for haystack in &haystacks {
				let Some((_end_score, new_typos, old_typos)) = analyze_contract(needle, haystack, &scoring) else {
					continue;
				};
				total_scored += 1;
				max_delta = max_delta.max(old_typos.saturating_sub(new_typos));

				for (b, &k) in budgets.iter().enumerate() {
					if old_typos <= k {
						old_accept[b] += 1;
					} else {
						old_reject[b] += 1;
						if new_typos <= k {
							new_recovered[b] += 1;
						}
					}
				}
			}
		}

		println!("typo_contract_tie_break_stats (old_det vs new_tie_aware)");
		println!("needles={needles:?} haystacks={} scored_pairs={total_scored}", haystacks.len());
		println!("max(old_typos - new_typos) observed: {max_delta}");
		for (b, &k) in budgets.iter().enumerate() {
			let accept = old_accept[b];
			let reject = old_reject[b];
			let recovered = new_recovered[b];
			let pct_of_rejected = if reject == 0 { 0.0 } else { (recovered as f64) * 100.0 / (reject as f64) };
			let pct_of_total = if total_scored == 0 {
				0.0
			} else {
				(recovered as f64) * 100.0 / (total_scored as f64)
			};
			println!(
				"budget k={k}: old_accept={accept} old_reject={reject} new_recovered={recovered} ({pct_of_rejected:.4}% of rejected, {pct_of_total:.6}% of total)"
			);
		}
	}
}
