# xeno-matcher

`xeno-matcher` is the fuzzy matching engine used by search, completion, and selection flows. Forked from Liam Dyer's [frizbee](https://github.com/saghen/frizbee).

## API surface

* `match_list` and `match_list_parallel` for bulk ranking
* `match_score` and `ScoreMatcher` for fast score-only checks
* `match_indices` for highlight positions on selected results
* `IncrementalMatcher` for typed-prefix workflows (`incremental` feature only)

## Scoring and typo contract

* Score uses a full-needle contract: best alignment that consumes the entire needle (`max` over the last needle row).
* Typo count in production is DP-consistent: `typos = needle_len - matched_chars`.
* `matched_chars` is tracked in lockstep with scoring DP winner selection.
* Winner selection is score-first with deterministic tie priority: `diag > up > left`.
* `exact_match_bonus` is applied after DP when `haystack == needle`.

This keeps score and typo gating aligned on the same winning path.

## Execution model

* Bulk and score-only paths dispatch by haystack-width buckets and run SIMD SW kernels (`smith_waterman::simd`).
* Prefiltering (`CandidateFilter`) can reject obvious non-candidates before SW.
* Large inputs fall back to greedy matching to bound work, using `match_too_large` for general SW size and `typo_sw_too_large` for typo-mode SW budget.
* Typo-mode SW uses a deterministic cell budget (`MAX_TYPO_SW_CELLS = 2048`).
* `match_list` and `match_list_parallel` preallocate result vectors to `haystacks.len()` to avoid typo-path realloc churn.
* `match_indices` uses the reference path with traceback indices for final rendering, not for scanning entire candidate sets.

## Typo counting implementations

### Production path

* `smith_waterman_scores_typos` in `smith_waterman::simd::algorithm` uses streaming forward DP.
* No score-matrix materialization.
* No traceback pass in hot path.

### Matrix-traceback tools

The crate also ships matrix-based typo tracebacks for debugging and experiments:

* `smith_waterman::reference::typos::typos_from_score_matrix`
* `smith_waterman::simd::typos::typos_from_score_matrix`

These implement tie-aware min-typo traversal over max-predecessor paths, plus a scalar-first strategy with SIMD-DP bailout for tie-heavy cases.

## Debugging and parity tooling

* `smith_waterman::debug::assert_sw_score_matrix_parity` provides a focused diff report for reference vs SIMD matrix divergence.
* `typo_contract_tie_break_stats` (ignored test) compares deterministic traceback vs tie-aware traceback to measure recovered candidates.

## Benchmark guardrails

Primary benchmark groups in `crates/matcher/benches/matcher.rs`:

* `match_list`
* `parallel_vs_serial`
* `incremental_typing` (`incremental` feature only)
* `prefilter_scan`
* `sw_micro`
* `match_list_typo_guardrails`

Commands:

```sh
cargo test -p xeno-matcher
cargo test -p xeno-matcher --features incremental
CC=gcc cargo bench -p xeno-matcher --bench matcher -- 'sw_micro|match_list_typo'
```

`CC=gcc` avoids the known bench-profile `clang+mold` LTO issue in this setup.

## Integration recipe

Typical UI flow:

* Phase 1: use `ScoreMatcher::score()` (or `match_list`) for fast ranking.
* Phase 2: call `match_indices()` only for top/visible rows to render highlights.
