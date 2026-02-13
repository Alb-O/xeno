# Matcher crate notes

## Benchmark command that currently works

Use GCC for Criterion benches in this environment:

`CC=gcc cargo bench -p xeno-matcher --bench matcher -- max_typos=Some`

This avoids the linker/LTO failure seen with the default `clang` + `mold` path (`alloca` build script emits `-flto` under clang).

Useful variants:

* Typo-focused serial view only: `CC=gcc cargo bench -p xeno-matcher --bench matcher -- match_list/serial`
* Incremental-only: `CC=gcc cargo bench -p xeno-matcher --bench matcher -- incremental_typing`

## Current benchmark rundown

Bench file: `crates/matcher/benches/matcher.rs`

* `match_list`: serial one-shot matching across 3 needles (`foo`, `deadbeef`, `serialfmt`) and typo configs (`None`, `Some(0)`, `Some(1)`) over 10k synthetic haystacks.
* `parallel_vs_serial`: compares `match_list` vs `match_list_parallel(..., 8)` for `deadbeef` across typo configs.
* `incremental_typing`: runs a realistic typing sequence (`"" -> "d" -> ... -> "deadbeef"`) with `max_typos=Some(1)`.
* `prefilter_scan`: isolates prefilter throughput for unordered matching at `max_typos=0` and typo-aware unordered matching at `max_typos=1`.

Recent local typo-path snapshot (with `CC=gcc`, Criterion):

* `match_list/serial/needle=deadbeef:max_typos=Some(0)`: about 0.73 ms
* `match_list/serial/needle=deadbeef:max_typos=Some(1)`: about 0.91 ms
* `match_list/serial/needle=serialfmt:max_typos=Some(0)`: about 0.54 ms
* `match_list/serial/needle=serialfmt:max_typos=Some(1)`: about 0.82 ms
* `incremental_typing`: about 78.6 ms

## Lesson learned: scratch-buffer reuse was the wrong optimization target

### Context

We fixed SIMD typo-mode correctness by switching from **streaming typo propagation** (single-pass, no full matrix) to **matrix materialization + traceback** (provably consistent with the scoring DP and cross-impl parity).

### What we tried

We attempted to speed up typo-mode by **reusing a `Vec` scratch score-matrix** (`*_in_place`), expecting fewer allocations to help incremental performance.

#### Variants attempted

* `clear() + resize()` / `resize() + fill()` → forces explicit `memset` over the entire buffer
* “no zero-fill” reuse → avoids memset but still keeps us on the matrix-materializing code path

### What we found

1. **Fresh allocation was “cheap” due to OS behavior**

   * `vec![[0; W]; n]` effectively behaves like `calloc` and benefits from **lazy zero pages**, so “allocate fresh” can be near-O(1) in practice.

2. **Reusing a dirty buffer is expensive if you zero it**

   * Any strategy that touches every byte (memset) is slower than the kernel’s lazy zeroing.

3. **The main cost wasn’t alloc vs reuse**

   * The real regression comes from:
     **streaming typo tracking (stack columns, no matrix)**
     → replaced by
     **matrix+traceback (heap NxW surface)**
   * Scratch reuse cannot undo the fundamental cost of materializing the full DP surface.

### Results

* Original pre-correctness code (streaming typos): ~68 ms (incremental_typing)
* Correct matrix+traceback + masking: ~71 ms
* Scratch reuse did not help; in some runs it made things worse or was within noise.
* Earlier ~58 ms was a warm-cache artifact, not a real baseline.

### What to avoid in the future

* Don’t chase “reuse Vec scratch buffers” when the alternative is a fresh `calloc`-like allocation.
* Don’t introduce explicit zeroing (memset) across large buffers in hot paths unless you must.
* Don’t expect scratch reuse to recover performance lost to an algorithmic shift (streaming → full matrix).

### What to do instead

* Keep correctness (matrix+traceback), then optimize by **skipping/short-circuiting** expensive work:

  * early-reject bounds (avoid traceback/matrix when definitely out of budget)
  * only materialize matrix for “maybe” candidates
  * keep unbanded score-first+gate semantics

If you want, I can format this as a ready-to-paste `MATCHER_NEXT.md` section header + bullet list.
