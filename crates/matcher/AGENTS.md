# Matcher crate notes

## Benchmark command that currently works

Use GCC for Criterion benches in this environment:

`CC=gcc cargo bench -p xeno-matcher --bench matcher -- max_typos=Some`

This avoids the linker/LTO failure seen with the default `clang` + `mold` path (`alloca` build script emits `-flto` under clang).

Useful variants:

- Typo-focused serial view only: `CC=gcc cargo bench -p xeno-matcher --bench matcher -- match_list/serial`
- Incremental-only: `CC=gcc cargo bench -p xeno-matcher --bench matcher -- incremental_typing`

## Current benchmark rundown

Bench file: `crates/matcher/benches/matcher.rs`

- `match_list`: serial one-shot matching across 3 needles (`foo`, `deadbeef`, `serialfmt`) and typo configs (`None`, `Some(0)`, `Some(1)`) over 10k synthetic haystacks.
- `parallel_vs_serial`: compares `match_list` vs `match_list_parallel(..., 8)` for `deadbeef` across typo configs.
- `incremental_typing`: runs a realistic typing sequence (`"" -> "d" -> ... -> "deadbeef"`) with `max_typos=Some(1)`.
- `prefilter_scan`: isolates prefilter throughput for unordered matching at `max_typos=0` and typo-aware unordered matching at `max_typos=1`.

Recent local typo-path snapshot (with `CC=gcc`, Criterion):

- `match_list/serial/needle=deadbeef:max_typos=Some(0)`: about 0.73 ms
- `match_list/serial/needle=deadbeef:max_typos=Some(1)`: about 0.91 ms
- `match_list/serial/needle=serialfmt:max_typos=Some(0)`: about 0.54 ms
- `match_list/serial/needle=serialfmt:max_typos=Some(1)`: about 0.82 ms
- `incremental_typing`: about 78.6 ms
