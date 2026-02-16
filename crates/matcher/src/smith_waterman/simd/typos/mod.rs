use core::simd::Simd;
use core::simd::prelude::*;

use multiversion::multiversion;

use crate::simd_lanes::{LaneCount, SupportedLaneCount};

/// Counts typos per lane via adaptive scalar-first traceback with SIMD DP bailout.
///
/// Starts with work-efficient scalar DFS per lane. Tracks visited states and bails
/// to a SIMD-vectorized forward DP if branching exceeds a visit budget (derived from
/// matrix size). This gives near-optimal performance for both ordered inputs (scalar
/// fast path) and tie-heavy inputs (bounded SIMD DP).
///
/// Row-0 is handled via closed form: `col + if m[col][0] == 0 { 1 } else { 0 }`.
/// Col-0 base: `if m[0][row] == 0 { 1 } else { 0 }`.
#[multiversion(targets(
    // x86-64-v4 without lahfsahf
    "x86_64+avx512f+avx512bw+avx512cd+avx512dq+avx512vl+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v3 without lahfsahf
    "x86_64+avx+avx2+bmi1+bmi2+cmpxchg16b+f16c+fma+fxsr+lzcnt+movbe+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3+xsave",
    // x86-64-v2 without lahfsahf
    "x86_64+cmpxchg16b+fxsr+popcnt+sse+sse2+sse3+sse4.1+sse4.2+ssse3",
))]
pub fn typos_from_score_matrix<const W: usize, const L: usize>(score_matrix: &[[Simd<u16, L>; W]], max_typos: u16) -> [u16; L]
where
	LaneCount<L>: SupportedLaneCount,
{
	let n = score_matrix.len();
	if n == 0 {
		return [0u16; L];
	}

	let last_scores = &score_matrix[n - 1];
	let mut end_scores = Simd::splat(0u16);
	for row in 0..W {
		end_scores = end_scores.simd_max(last_scores[row]);
	}

	let (mut out, bailed) = typos_scalar::<W, L>(score_matrix, max_typos, end_scores);

	if bailed.iter().any(|&b| b) {
		let dp = typos_simd_dp::<W, L>(score_matrix, max_typos, end_scores);
		for lane in 0..L {
			if bailed[lane] {
				out[lane] = dp[lane];
			}
		}
	}

	out
}

/// Scalar per-lane DFS traceback with Vec memo and visit-budget bailout.
fn typos_scalar<const W: usize, const L: usize>(score_matrix: &[[Simd<u16, L>; W]], max_typos: u16, end_scores: Simd<u16, L>) -> ([u16; L], [bool; L])
where
	LaneCount<L>: SupportedLaneCount,
{
	let mut out = [0u16; L];
	let mut bailed = [false; L];
	let n = score_matrix.len();
	let cap = max_typos.saturating_add(1);
	let end_scores_arr = end_scores.to_array();
	let last = &score_matrix[n - 1];

	let cells = n * W;
	let visit_limit = (cells / 4).clamp(128, 256);

	let mut memo = vec![u16::MAX; n * W];

	for lane in 0..L {
		let end_score = end_scores_arr[lane];
		if end_score == 0 {
			let t = n as u16;
			out[lane] = if t > max_typos { cap } else { t };
			continue;
		}

		memo.fill(u16::MAX);
		let col = n - 1;
		let mut best = u16::MAX;
		let mut visited = 0usize;
		let mut lane_bailed = false;
		for row in 0..W {
			if last[row][lane] != end_score {
				continue;
			}
			best = best.min(traceback_lane::<W, L>(
				score_matrix,
				lane,
				col,
				row,
				cap,
				&mut memo,
				&mut visited,
				visit_limit,
				&mut lane_bailed,
			));
			if best == 0 || lane_bailed {
				break;
			}
		}
		if lane_bailed {
			bailed[lane] = true;
		} else {
			out[lane] = best.min(cap);
		}
	}
	(out, bailed)
}

fn traceback_lane<const W: usize, const L: usize>(
	m: &[[Simd<u16, L>; W]],
	lane: usize,
	col: usize,
	row: usize,
	cap: u16,
	memo: &mut [u16],
	visited: &mut usize,
	visit_limit: usize,
	bailed: &mut bool,
) -> u16
where
	LaneCount<L>: SupportedLaneCount,
{
	if *bailed {
		return cap;
	}
	if row == 0 {
		return (col as u16 + if m[col][0][lane] == 0 { 1 } else { 0 }).min(cap);
	}
	if col == 0 {
		return if m[0][row][lane] == 0 { 1 } else { 0 };
	}
	let idx = col * W + row;
	if memo[idx] != u16::MAX {
		return memo[idx];
	}

	*visited += 1;
	if *visited > visit_limit {
		*bailed = true;
		return cap;
	}

	let cur = m[col][row][lane];
	let diag = m[col - 1][row - 1][lane];
	let left = m[col - 1][row][lane];
	let up = m[col][row - 1][lane];
	let max_prev = diag.max(left).max(up);
	let mut best = cap;
	if diag == max_prev {
		let add: u16 = if diag >= cur { 1 } else { 0 };
		let v = add.saturating_add(traceback_lane::<W, L>(m, lane, col - 1, row - 1, cap, memo, visited, visit_limit, bailed));
		best = best.min(v.min(cap));
	}
	if left == max_prev && !*bailed {
		let v = 1u16.saturating_add(traceback_lane::<W, L>(m, lane, col - 1, row, cap, memo, visited, visit_limit, bailed));
		best = best.min(v.min(cap));
	}
	if up == max_prev && !*bailed {
		let v = traceback_lane::<W, L>(m, lane, col, row - 1, cap, memo, visited, visit_limit, bailed);
		best = best.min(v.min(cap));
	}
	if !*bailed {
		memo[idx] = best;
	}
	best
}

/// SIMD-vectorized forward DP. All lanes processed simultaneously. Bounded O(n*W) work.
fn typos_simd_dp<const W: usize, const L: usize>(score_matrix: &[[Simd<u16, L>; W]], max_typos: u16, end_scores: Simd<u16, L>) -> [u16; L]
where
	LaneCount<L>: SupportedLaneCount,
{
	let n = score_matrix.len();
	let cap = max_typos.saturating_add(1);
	let cap_s = Simd::splat(cap);
	let zero = Simd::splat(0u16);
	let one = Simd::splat(1u16);

	let last_scores = &score_matrix[n - 1];

	let mut prev = [Simd::splat(0u16); W];
	let mut curr = [Simd::splat(0u16); W];

	for row in 0..W {
		let is_zero = score_matrix[0][row].simd_eq(zero);
		prev[row] = is_zero.select(one, zero);
	}

	for col in 1..n {
		let base = Simd::splat(col as u16);
		let extra = score_matrix[col][0].simd_eq(zero).select(one, zero);
		curr[0] = base.saturating_add(extra).simd_min(cap_s);

		for row in 1..W {
			let cur_score = score_matrix[col][row];
			let diag = score_matrix[col - 1][row - 1];
			let left = score_matrix[col - 1][row];
			let up = score_matrix[col][row - 1];
			let max_prev = diag.simd_max(left).simd_max(up);

			let diag_cost = {
				let allowed = diag.simd_eq(max_prev);
				let add = diag.simd_ge(cur_score).select(one, zero);
				let cost = prev[row - 1].saturating_add(add).simd_min(cap_s);
				allowed.select(cost, cap_s)
			};

			let left_cost = {
				let allowed = left.simd_eq(max_prev);
				let cost = prev[row].saturating_add(one).simd_min(cap_s);
				allowed.select(cost, cap_s)
			};

			let up_cost = {
				let allowed = up.simd_eq(max_prev);
				allowed.select(curr[row - 1], cap_s)
			};

			curr[row] = diag_cost.simd_min(left_cost).simd_min(up_cost);
		}

		std::mem::swap(&mut prev, &mut curr);
	}

	let mut best = cap_s;
	for row in 0..W {
		let mask = last_scores[row].simd_eq(end_scores);
		let cand = mask.select(prev[row], cap_s);
		best = best.simd_min(cand);
	}

	let n_typos = (n as u16).min(cap);
	let end_zero = end_scores.simd_eq(zero);
	best = end_zero.select(Simd::splat(n_typos), best);

	best.to_array()
}

#[cfg(test)]
mod tests;
