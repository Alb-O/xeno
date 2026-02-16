## Scoring contract

Full-needle contract: score = max over the last needle row. The score represents
the best alignment that consumes the entire needle, not a local maximum anywhere
in the matrix.

Typo semantics: `typos = needle_len - matched_chars`, where `matched_chars` is
the number of diagonal-match steps on the winning path. Tracked via streaming
forward DP in lockstep with the scoring DP using identical winner selection.
No matrix allocation or traceback pass needed.

Winner selection (tie-break priority: diag > up > left, ties included):
diag wins if `diag_score >= up_score && diag_score >= left_score`;
up wins if `!diag_wins && up_score >= left_score`; left wins otherwise.
Same in reference scalar and SIMD streaming paths.

## Implementation map

* Reference scalar: `smith_waterman::reference::algorithm::smith_waterman`
  Returns `(score, typos, score_matrix, exact)` with matched-count tracking.
* SIMD score-only: `smith_waterman::simd::algorithm::smith_waterman_scores`
  Full-needle contract, no typo tracking.
* SIMD score+typos: `smith_waterman::simd::algorithm::smith_waterman_scores_typos`
  Streaming matched-count DP alongside scores. No `Vec` matrix allocation.
* Single-haystack scorer: `one_shot::score::ScoreMatcher` / `match_score`
  SIMD L=1 dispatch with bucket width selection, CandidateFilter reuse.
* One-shot indices: `one_shot::indices::match_indices`
  Uses reference SW for score matrix + traceback, DP-consistent typos.
* Bulk matching: `one_shot::match_list` / `match_list_parallel`
  Uses SIMD kernels via `kernels::fixed_width`.
* Incremental: Feature-gated behind `incremental`. Not used by editor.

## Editor integration

File search uses 2-phase scoring:
* Phase 1: `ScoreMatcher::score()` for fast SIMD top-K heap scan.
* Phase 2: `match_indices()` only for final top-K results (highlight indices).

## Old heuristic traceback

`typos_from_score_matrix` (reference and SIMD) is demoted to `#[cfg(test)]` only.
It infers the alignment path from the score matrix alone, which is underdetermined
with affine gaps. The streaming matched-count DP is the authoritative typo source.
