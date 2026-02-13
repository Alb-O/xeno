### State of the crate after this session

**What’s now clean / de-duplicated**

* **Bucket spec is single-source-of-truth** via `for_each_bucket_spec!` in `lib.rs`.
* **Shared policy helpers** are centralized in `limits.rs` (no more incremental→one_shot coupling).
* **Incremental buckets are typed** (no `Box<dyn …>`, no vtable per bucket).
* **Fixed-width execution is unified** in `kernels::fixed_width::emit_fixed_width_matches`:

  * score-only SIMD
  * typo-mode SIMD
  * typo SW budget fallback → greedy
  * typo gating (via `exceeds_typo_budget`)

**Pipeline primitives now exist**

* `engine::CandidateFilter` encapsulates:

  * `min_haystack_len`
  * optional prefilter setup and decision logic
* One-shot and incremental overflow both use this filter consistently.

**Overflow path**

* Incremental no longer calls `crate::match_list` for overflow strings.
* Overflow uses filter + greedy + typo gate directly.
* Parity test exists for `len > 512`.

### Current blocker: SIMD typo-mode correctness (score parity)

A new parity test was added:

* `smith_waterman/reference/algorithm.rs::simd_typo_counts_and_gating_match_reference`

It shows **typo-mode SIMD score mismatches reference** even when:

* `ref_typos <= max_typos`
* `simd_typos == ref_typos`

Example failing case:

* `needle="Dd/aAd", haystack="da--aD-ca/c-", max_typos=3`
* `ref_score=65, simd_score=47`

Also, an existing test now fails:

* `smith_waterman::simd::typos::tests::streaming_typos_matches_matrix_traceback`

  * score mismatch on case 0 max_typos=0

This strongly suggests: **`smith_waterman_scores_typos` is not computing the same scoring DP as the normal SIMD SW**, likely due to interaction between:

* DP banding/full-width traversal
* delimiter bonus enablement state
* and/or the separate “typo tracking” logic choosing predecessors using the wrong comparison basis (it compares raw `diag/left/up` base values, not the actual transition scores with penalties/bonuses).

### Recommended next steps (in order)

#### Ticket 1 — Make typo-mode score DP share the exact same scoring path as score-only SIMD

Goal: guarantee `scores_typos` uses the *same* scoring DP as `smith_waterman_scores` (only additional metadata differs).

Concrete approach:

* In `smith_waterman_scores_typos`, compute the score matrix exactly like `smith_waterman`/`smith_waterman_scores`:

  * same row init, same `smith_waterman_inner`, same max score accumulation.
* Treat typo accounting as **separate** from score DP:

  * either compute typos from the resulting score matrix using a deterministic traceback-like pass, or
  * compute additional predecessor metadata during DP that matches the DP’s actual argmax decisions (must include penalties/bonuses).

If you keep “typo tracking” as a second pass:

* define how typos are measured (current intended meaning: `needle_len - matched_needle_chars`).
* ensure tie-breaking is identical to reference or at least deterministic and consistent.

#### Ticket 2 — Decide the contract: “maximize score subject to typos<=k” vs “maximize score, then filter by typos”

Currently the system effectively does the latter. If you want *correct constrained matching*, typo must participate in the DP objective (lexicographic optimization or constrained DP).
Pick one:

* **Keep current contract** (score-first then filter): easier, but can reject a valid in-budget match because the best-score alignment is out-of-budget.
* **Upgrade contract**: maximize `(score, -typos)` or maximize score with hard typo constraint. More work, but principled.

Do not proceed until this is written down; tests depend on it.

#### Ticket 3 — Debugging harness improvements

* Keep the failing parity test; reduce it to a minimal reproducer for the current mismatch.
* Add a helper to print:

  * per-cell DP score for both reference and SIMD for a single lane (W=16/L=1)
  * exact location where divergence begins (needle_idx, haystack_idx)
    This will pinpoint whether divergence is in:
* delimiter bonus enablement
* offset-prefix handling
* capitalization bonus
* gap penalty masks / transitions
* mismatch penalty application

### Guardrails / invariants to keep

* Greedy fallback must honor `Scoring.delimiters` and never panic on short haystacks.
* `match_list_parallel == match_list` across typo configs.
* Incremental parity with one-shot across typing sequences, including overflow.
* CandidateFilter must never introduce false negatives vs full matcher.
* New typo-mode SIMD parity test stays until green.
