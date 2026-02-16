# Matcher: State of the World

## Typo Contract

**Decision: score-first-then-gate with tie-aware traceback.**

* Compute Smith-Waterman scores (same DP as score-only mode).
* Count typos via deterministic traceback over the score matrix.
* Reject candidates where `typos > k` (`max_typos = Some(k)`).

Among equally best-score alignments, the traceback explores all max-predecessor
paths and picks the one with minimum typo count. This eliminates false negatives
from arbitrary tie-breaking (~0.4% recovered at k=1,2 on typical workloads).

This is **not** constrained DP — we do not maximize score subject to typos<=k.
A lower-score alignment that happens to be in-budget may still be rejected if a
higher-score alignment exceeds the budget. That's intentional: scores are the
primary ranking signal.

## Typo Traceback Implementation

### Reference (`smith_waterman::reference::typos`)

Tie-aware DFS with flat `Vec<u16>` memo keyed by `(col, row)`:

* Row-0 closed form: `col + if m[col][0] == 0 { 1 } else { 0 }` (forced left
  moves never branch).
* Col-0 base: `if m[0][row] == 0 { 1 } else { 0 }`.
* For row > 0: running score equals `m[col][row]`, so the mismatch test uses
  cell values directly — no score parameter needed in memo key.

### SIMD (`smith_waterman::simd::typos`)

Adaptive scalar-first with SIMD DP bailout:

1. **Scalar DFS** per lane with `Vec<u16>` memo and visit-budget counter.
   Same algorithm as reference. Budget = `clamp(n*W/4, 128, 256)`.
2. If any lane exceeds the visit budget → **bail to SIMD forward DP** which
   processes all lanes simultaneously via `Simd<u16, L>` operations. Bounded
   O(n*W) work regardless of tie density.
3. Patch bailed lanes with DP results; keep scalar results for non-bailed lanes.

**Why not always SIMD DP?** The forward DP does a full n*W pass even for simple
inputs where the scalar DFS visits only a small fraction of cells. On ordered
inputs, scalar is ~30% faster than the DP.

**Why not a static heuristic (e.g. end-tie count)?** End-position tie count
doesn't predict internal traceback branching. A haystack can have 1 end position
but dense max-predecessor plateaus that cause DFS explosion. The visit budget
directly measures the pathological behavior.

## Score Matrix Materialization

Full score matrix is only materialized under the typo SW budget
(`MAX_TYPO_SW_CELLS` = `needle_len * W`). Beyond that threshold, the greedy
fallback is used (no matrix, no traceback).

## Test Harness

### Stats harness

```sh
cargo test -p xeno-matcher typo_contract_tie_break_stats -- --ignored --nocapture
```

Compares old deterministic traceback (fixed diag>left>up priority) vs new
tie-aware traceback across random needle/haystack pairs. Key output:

* `new_recovered`: candidates that the old traceback rejected but the new one
  accepts at the same budget. This quantifies false negatives eliminated.

### Debug tooling

`smith_waterman::debug::assert_sw_score_matrix_parity` — panics with a detailed
diff report (cell coordinates, score windows, per-component explanation) on any
reference vs SIMD score divergence. Integrated into the fuzz parity tests.

## Bench Guardrails

```sh
CC=gcc cargo bench -p xeno-matcher --bench matcher -- 'sw_micro|match_list_typo'
```

**Note:** The default `clang+mold` linker config hits an LTO error with the
`alloca` crate in bench profile. Use `CC=gcc` as a workaround.

### `sw_micro`

Microbench of `smith_waterman_scores` vs `smith_waterman_scores_typos` at W=64
and W=256 with ordered (needle embedded) and tie-heavy (delimiter gap + repeated
tail) haystacks. Needle = `"deadbeef"` (len=8).

* **Ordered**: typo traceback adds ~7-15% over score-only (scalar fast path).
* **Tie-heavy**: ~1.9x faster than the old deterministic traceback at W=256
  (visit-budget bailout → SIMD DP).

### `match_list_typo_guardrails`

End-to-end `match_list` with 2000 haystacks at W=256, k=1:

* `"deadbeef"` (len=8) → typo SW path.
* `"serialfmt"` (len=9) → greedy fallback via `typo_sw_too_large`.

Baseline numbers (as of this writing):

* deadbeef SW path: ~1.9 ms
* serialfmt greedy: ~520 µs (3.6x cheaper — boundary is working)

## Allocation

`match_list` and `match_list_parallel` unconditionally preallocate
`Vec::with_capacity(haystacks.len())` for the results vec. Previously,
typo-mode used `vec![]` which caused repeated realloc on large lists.
