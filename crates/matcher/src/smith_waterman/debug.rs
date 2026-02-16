//! Test-only SW parity diff reporter.
//!
//! When reference and SIMD score matrices diverge, prints the first divergence
//! cell coordinates, surrounding score windows, and a per-component breakdown
//! for both implementations so the root cause is immediately visible.

use std::fmt::Write as _;

use crate::Scoring;

#[derive(Debug, Clone)]
struct CellExplain {
	impl_name: &'static str,
	i: usize,
	j: usize,
	needle_b: u8,
	hay_b: u8,
	needle_lower: u8,
	hay_lower: u8,
	is_match: bool,
	matched_casing: bool,
	diag: u16,
	left: u16,
	up_prev: u16,
	up_gap_open: bool,
	left_gap_open: bool,
	up_gap_penalty: u16,
	left_gap_penalty: u16,
	prefix: bool,
	offset_prefix: bool,
	capitalization_bonus: u16,
	delimiter_bonus: u16,
	matching_case_bonus: u16,
	match_score_term: u16,
	diag_score: u16,
	up_score: u16,
	left_score: u16,
	max_score: u16,
	diag_is_max: bool,
	up_is_max: bool,
	left_is_max: bool,
	delimiter_bonus_enabled: bool,
	prev_hay_is_delimiter: bool,
	prev_hay_is_lower: bool,
}

/// Panics with a detailed diff report if reference and SIMD score matrices diverge.
pub(crate) fn assert_sw_score_matrix_parity(needle: &str, haystack: &str, scoring: &Scoring) {
	if let Some(report) = sw_score_matrix_diff_report(needle, haystack, scoring) {
		panic!("{report}");
	}
}

/// Returns `Some(report)` when ref vs SIMD score matrices differ, `None` when identical.
pub(crate) fn sw_score_matrix_diff_report(needle: &str, haystack: &str, scoring: &Scoring) -> Option<String> {
	if needle.is_empty() || haystack.is_empty() {
		return None;
	}
	if haystack.len() > 512 {
		return Some(format!(
			"sw_score_matrix_diff_report only supports haystack.len()<=512 (got {})",
			haystack.len()
		));
	}

	macro_rules! dispatch {
		($(($name:ident, $width:literal, $range:pat)),* $(,)?) => {{
			match haystack.len() {
				$($range => sw_score_matrix_diff_report_with::<$width>(needle, haystack, scoring),)*
				_ => unreachable!("haystack.len()<=512 but did not match bucket ranges"),
			}
		}};
	}
	crate::for_each_bucket_spec!(dispatch)
}

fn sw_score_matrix_diff_report_with<const W: usize>(needle: &str, haystack: &str, scoring: &Scoring) -> Option<String> {
	let (_ref_score, _ref_typos, ref_matrix, _ref_exact) = crate::smith_waterman::reference::smith_waterman(needle, haystack, scoring);
	let hs = [haystack];
	let (_simd_scores, simd_matrix, _simd_exact) = crate::smith_waterman::simd::smith_waterman::<W, 1>(needle, &hs, None, scoring);

	let needle_bytes = needle.as_bytes();
	let hay_bytes = haystack.as_bytes();
	let n = needle_bytes.len();
	let h = hay_bytes.len();

	debug_assert_eq!(ref_matrix.len(), n);
	debug_assert!(ref_matrix.iter().all(|r| r.len() == h));

	let simd_rows: Vec<Vec<u16>> = simd_matrix.iter().map(|row| (0..h).map(|j| row[j][0]).collect::<Vec<u16>>()).collect();

	let mut first: Option<(usize, usize)> = None;
	'outer: for i in 0..n {
		for j in 0..h {
			if ref_matrix[i][j] != simd_rows[i][j] {
				first = Some((i, j));
				break 'outer;
			}
		}
	}

	let (i, j) = first?;

	let mut out = String::new();
	let nb = needle_bytes[i];
	let hb = hay_bytes[j];

	writeln!(&mut out, "SW PARITY DIVERGENCE").ok();
	writeln!(&mut out, "needle={needle:?} (len={n})").ok();
	writeln!(&mut out, "haystack={haystack:?} (len={h})").ok();
	writeln!(&mut out, "bucket W={W}").ok();
	writeln!(
		&mut out,
		"scoring: match={} mismatch_penalty={} gap_open={} gap_extend={} prefix={} offset_prefix={} cap_bonus={} case_bonus={} exact_bonus={} delimiter_bonus={} delimiters={:?}",
		scoring.match_score,
		scoring.mismatch_penalty,
		scoring.gap_open_penalty,
		scoring.gap_extend_penalty,
		scoring.prefix_bonus,
		scoring.offset_prefix_bonus,
		scoring.capitalization_bonus,
		scoring.matching_case_bonus,
		scoring.exact_match_bonus,
		scoring.delimiter_bonus,
		scoring.delimiters,
	)
	.ok();
	writeln!(&mut out).ok();
	writeln!(&mut out, "first mismatch: i={i} j={j} ref={} simd={}", ref_matrix[i][j], simd_rows[i][j]).ok();
	write!(&mut out, "bytes: needle[i]=").ok();
	push_byte(&mut out, nb);
	write!(&mut out, " haystack[j]=").ok();
	push_byte(&mut out, hb);
	writeln!(&mut out).ok();

	let row_radius = 6usize;
	writeln!(&mut out).ok();
	push_row_window(&mut out, "ref  row[i] ", &ref_matrix[i], j, row_radius);
	push_row_window(&mut out, "simd row[i] ", &simd_rows[i], j, row_radius);
	if i > 0 {
		push_row_window(&mut out, "ref  row[i-1] ", &ref_matrix[i - 1], j, row_radius);
		push_row_window(&mut out, "simd row[i-1] ", &simd_rows[i - 1], j, row_radius);
	}

	writeln!(&mut out).ok();
	let ref_ex = explain_reference_cell(needle, haystack, scoring, i, j);
	let simd_ex = explain_simd_scoring_cell(needle, haystack, scoring, i, j);
	push_cell_explain(&mut out, &ref_ex);
	writeln!(&mut out).ok();
	push_cell_explain(&mut out, &simd_ex);

	Some(out)
}

fn push_row_window(out: &mut String, label: &str, row: &[u16], j: usize, radius: usize) {
	let start = j.saturating_sub(radius);
	let end = (j + radius + 1).min(row.len());
	write!(out, "{label}[{start}..{end}]:").ok();
	for value in row.iter().take(end).skip(start) {
		write!(out, " {:>4}", value).ok();
	}
	writeln!(out).ok();
}

fn push_byte(out: &mut String, b: u8) {
	if b.is_ascii_graphic() || b == b' ' {
		write!(out, "'{}' (0x{:02x})", b as char, b).ok();
	} else {
		write!(out, "0x{:02x}", b).ok();
	}
}

fn delimiter_table(scoring: &Scoring) -> [bool; 256] {
	let mut t = [false; 256];
	for d in scoring.delimiters.bytes() {
		t[d.to_ascii_lowercase() as usize] = true;
	}
	t
}

fn explain_reference_cell(needle: &str, haystack: &str, scoring: &Scoring, target_i: usize, target_j: usize) -> CellExplain {
	let needle = needle.as_bytes();
	let hay = haystack.as_bytes();
	let n = needle.len();
	let h = hay.len();
	let delims = delimiter_table(scoring);

	let mut prev_row = vec![0u16; h];
	let mut curr_row = vec![0u16; h];

	for (i, &needle_b) in needle.iter().enumerate().take(n) {
		let needle_is_upper = needle_b.is_ascii_uppercase();
		let needle_lower = needle_b.to_ascii_lowercase();
		let mut up_score_prev: u16 = 0;
		let mut up_gap_open = true;
		let mut left_gap_open = true;
		let mut delimiter_bonus_enabled = false;
		let mut prev_hay_is_delimiter = false;
		let mut prev_hay_is_lower = false;

		for j in 0..h {
			let prefix = j == 0;
			let offset_prefix = j == 1 && prev_row[0] == 0 && !hay[0].is_ascii_alphabetic();
			let hay_b = hay[j];
			let hay_is_upper = hay_b.is_ascii_uppercase();
			let hay_is_lower = hay_b.is_ascii_lowercase();
			let hay_lower = hay_b.to_ascii_lowercase();
			let hay_is_delimiter = delims[hay_lower as usize];
			let matched_casing = needle_is_upper == hay_is_upper;

			let match_score_term = if prefix {
				scoring.match_score + scoring.prefix_bonus
			} else if offset_prefix {
				scoring.match_score + scoring.offset_prefix_bonus
			} else {
				scoring.match_score
			};
			let diag = if prefix { 0 } else { prev_row[j - 1] };
			let left = prev_row[j];
			let is_match = needle_lower == hay_lower;

			let delimiter_bonus = if is_match && prev_hay_is_delimiter && delimiter_bonus_enabled && !hay_is_delimiter {
				scoring.delimiter_bonus
			} else {
				0
			};
			let capitalization_bonus = if is_match && !prefix && hay_is_upper && prev_hay_is_lower {
				scoring.capitalization_bonus
			} else {
				0
			};
			let matching_case_bonus = if is_match && matched_casing { scoring.matching_case_bonus } else { 0 };
			let diag_score = if is_match {
				diag + match_score_term + delimiter_bonus + capitalization_bonus + matching_case_bonus
			} else {
				diag.saturating_sub(scoring.mismatch_penalty)
			};
			let up_gap_penalty = if up_gap_open { scoring.gap_open_penalty } else { scoring.gap_extend_penalty };
			let up_score = up_score_prev.saturating_sub(up_gap_penalty);
			let left_gap_penalty = if left_gap_open {
				scoring.gap_open_penalty
			} else {
				scoring.gap_extend_penalty
			};
			let left_score = left.saturating_sub(left_gap_penalty);
			let max_score = diag_score.max(up_score).max(left_score);
			let diag_is_max = max_score == diag_score;
			let up_is_max = max_score == up_score;
			let left_is_max = max_score == left_score;

			if i == target_i && j == target_j {
				return CellExplain {
					impl_name: "reference",
					i,
					j,
					needle_b,
					hay_b,
					needle_lower,
					hay_lower,
					is_match,
					matched_casing,
					diag,
					left,
					up_prev: up_score_prev,
					up_gap_open,
					left_gap_open,
					up_gap_penalty,
					left_gap_penalty,
					prefix,
					offset_prefix,
					capitalization_bonus,
					delimiter_bonus,
					matching_case_bonus,
					match_score_term,
					diag_score,
					up_score,
					left_score,
					max_score,
					diag_is_max,
					up_is_max,
					left_is_max,
					delimiter_bonus_enabled,
					prev_hay_is_delimiter,
					prev_hay_is_lower,
				};
			}

			let diag_mask = diag_is_max;
			up_gap_open = max_score != up_score || diag_mask;
			left_gap_open = max_score != left_score || diag_mask;
			prev_hay_is_lower = hay_is_lower;
			prev_hay_is_delimiter = hay_is_delimiter;
			delimiter_bonus_enabled |= !prev_hay_is_delimiter;
			up_score_prev = max_score;
			curr_row[j] = max_score;
		}

		if i == target_i {
			break;
		}
		prev_row.clone_from(&curr_row);
		curr_row.fill(0);
	}
	panic!("explain_reference_cell: target out of range i={target_i} j={target_j}");
}

fn explain_simd_scoring_cell(needle: &str, haystack: &str, scoring: &Scoring, target_i: usize, target_j: usize) -> CellExplain {
	let needle = needle.as_bytes();
	let hay = haystack.as_bytes();
	let n = needle.len();
	let h = hay.len();
	let delims = delimiter_table(scoring);

	let mut prev_row = vec![0u16; h];
	let mut curr_row = vec![0u16; h];

	for (i, &needle_b) in needle.iter().enumerate().take(n) {
		let needle_is_capital = needle_b.is_ascii_uppercase();
		let needle_lower = needle_b.to_ascii_lowercase();
		let mut up_score_prev: u16 = 0;
		let mut up_gap_open = true;
		let mut left_gap_open = true;
		let mut delimiter_bonus_enabled = false;
		let mut prev_hay_is_delimiter = false;
		let mut prev_hay_is_lower = false;

		for j in 0..h {
			let hay_b = hay[j];
			let hay_is_capital = hay_b.is_ascii_uppercase();
			let hay_is_lower = hay_b.is_ascii_lowercase();
			let hay_lower = hay_b.to_ascii_lowercase();
			let hay_is_delimiter = delims[hay_lower as usize];
			let matched_casing = needle_is_capital == hay_is_capital;

			let diag = if j == 0 { 0 } else { prev_row[j - 1] };
			let left = prev_row[j];
			let is_match = needle_lower == hay_lower;

			let prefix = j == 0;
			let offset_prefix = j == 1 && diag == 0 && !(hay[0].is_ascii_lowercase() || hay[0].is_ascii_uppercase());

			let capitalization_bonus = if j > 0 && hay_is_capital && prev_hay_is_lower {
				scoring.capitalization_bonus
			} else {
				0
			};
			let delimiter_bonus = if j > 0 && prev_hay_is_delimiter && delimiter_bonus_enabled && !hay_is_delimiter {
				scoring.delimiter_bonus
			} else {
				0
			};

			let match_score_term = if prefix {
				scoring.prefix_bonus + scoring.match_score
			} else if offset_prefix {
				scoring.offset_prefix_bonus + scoring.match_score
			} else {
				scoring.match_score + capitalization_bonus + delimiter_bonus
			};
			let matching_case_bonus = if is_match && matched_casing { scoring.matching_case_bonus } else { 0 };
			let diag_score = if is_match {
				diag + matching_case_bonus + match_score_term
			} else {
				diag.saturating_sub(scoring.mismatch_penalty)
			};
			let up_gap_penalty = if up_gap_open { scoring.gap_open_penalty } else { scoring.gap_extend_penalty };
			let up_score = up_score_prev.saturating_sub(up_gap_penalty);
			let left_gap_penalty = if left_gap_open {
				scoring.gap_open_penalty
			} else {
				scoring.gap_extend_penalty
			};
			let left_score = left.saturating_sub(left_gap_penalty);
			let max_score = diag_score.max(up_score).max(left_score);
			let diag_is_max = max_score == diag_score;
			let up_is_max = max_score == up_score;
			let left_is_max = max_score == left_score;

			if i == target_i && j == target_j {
				return CellExplain {
					impl_name: "simd_scoring_formula",
					i,
					j,
					needle_b,
					hay_b,
					needle_lower,
					hay_lower,
					is_match,
					matched_casing,
					diag,
					left,
					up_prev: up_score_prev,
					up_gap_open,
					left_gap_open,
					up_gap_penalty,
					left_gap_penalty,
					prefix,
					offset_prefix,
					capitalization_bonus,
					delimiter_bonus,
					matching_case_bonus,
					match_score_term,
					diag_score,
					up_score,
					left_score,
					max_score,
					diag_is_max,
					up_is_max,
					left_is_max,
					delimiter_bonus_enabled,
					prev_hay_is_delimiter,
					prev_hay_is_lower,
				};
			}

			let diag_mask = diag_is_max;
			up_gap_open = max_score != up_score || diag_mask;
			left_gap_open = max_score != left_score || diag_mask;
			delimiter_bonus_enabled |= !hay_is_delimiter;
			prev_hay_is_delimiter = hay_is_delimiter;
			prev_hay_is_lower = hay_is_lower;
			up_score_prev = max_score;
			curr_row[j] = max_score;
		}

		if i == target_i {
			break;
		}
		prev_row.clone_from(&curr_row);
		curr_row.fill(0);
	}
	panic!("explain_simd_scoring_cell: target out of range i={target_i} j={target_j}");
}

fn push_cell_explain(out: &mut String, e: &CellExplain) {
	writeln!(out, "== {} cell i={} j={} ==", e.impl_name, e.i, e.j).ok();
	write!(out, "needle[i]=").ok();
	push_byte(out, e.needle_b);
	write!(out, " (lower=").ok();
	push_byte(out, e.needle_lower);
	writeln!(out, ")").ok();
	write!(out, "haystack[j]=").ok();
	push_byte(out, e.hay_b);
	write!(out, " (lower=").ok();
	push_byte(out, e.hay_lower);
	writeln!(out, ")").ok();
	writeln!(
		out,
		"match={} matched_casing={} prefix={} offset_prefix={}",
		e.is_match, e.matched_casing, e.prefix, e.offset_prefix
	)
	.ok();
	writeln!(
		out,
		"state(before): up_prev={} up_gap_open={} left_gap_open={} delim_enabled={} prev_delim={} prev_lower={}",
		e.up_prev, e.up_gap_open, e.left_gap_open, e.delimiter_bonus_enabled, e.prev_hay_is_delimiter, e.prev_hay_is_lower
	)
	.ok();
	writeln!(out, "inputs: diag={} left={}", e.diag, e.left).ok();
	writeln!(out, "penalties: up_gap_penalty={} left_gap_penalty={}", e.up_gap_penalty, e.left_gap_penalty).ok();
	writeln!(
		out,
		"bonuses: cap={} delim={} case={}",
		e.capitalization_bonus, e.delimiter_bonus, e.matching_case_bonus
	)
	.ok();
	writeln!(out, "match_score_term={}", e.match_score_term).ok();
	writeln!(
		out,
		"candidates: diag_score={} up_score={} left_score={} => max={} (diag_max={} up_max={} left_max={})",
		e.diag_score, e.up_score, e.left_score, e.max_score, e.diag_is_max, e.up_is_max, e.left_is_max
	)
	.ok();
}

#[test]
fn sw_debug_helper_smoke() {
	let scoring = Scoring::default();
	assert!(sw_score_matrix_diff_report("deadbeef", "___deadbeef___", &scoring).is_none());
}
