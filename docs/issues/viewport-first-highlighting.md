# Viewport-First Highlighting for L-tier Files

Status: active on `main` (not blocked)  
Last verified: 2026-02-10

## Summary

Viewport-first parsing is now wired through the syntax scheduler for L-tier files.
The earlier "blocked" state (incorrect trees from raw truncation) was mitigated by
sealing viewport windows before parse and then promoting to full parse in the
background.

## Current architecture

- `crates/editor/src/syntax_manager/ensure.rs`:
  - schedules `TaskKind::ViewportParse` for visible L-tier docs (Stage A)
  - can run optional Stage B over the same coverage with injections enabled when budget allows
  - marks viewport trees dirty so a full parse still follows
- `crates/editor/src/syntax_manager/tasks.rs`:
  - constructs viewport parse input as a `SealedSource` window
  - runs `ViewportRepair::scan` to append a synthetic closer or extend to a real closer
- `crates/language/src/syntax/mod.rs`:
  - represents viewport trees via `Syntax::new_viewport(...)`
  - maps highlight output back to document-global offsets
- `crates/language/src/language/mod.rs`:
  - derives repair rules from language metadata (or defaults) for comments/strings

## Historical blocker

The original blocker is still valid as a historical note: plain truncation and
`set_included_ranges` can produce invalid trees around multi-line constructs.
Current code avoids that path by sealing the window first.

## Remaining gaps

- `ViewportRepair::scan` still has a known TODO for delimiters that span chunk boundaries.
- Repair is heuristic, not grammar-complete; uncommon delimiter forms may still need per-language overrides.
- There is no dedicated unit-test suite for repair scanner edge cases (scheduler tests exist, but parser-boundary fixtures are still thin).
- There is no benchmark gate in CI for large-file first-highlight latency/regression.

## Suggested follow-ups

1. Add focused scanner tests for boundary cases in `crates/language/src/syntax/tests.rs`.
2. Add fixture-based integration tests for top-of-file multiline comment/string cases on large files.
3. Add a repeatable benchmark scenario (for example `tmp/miniaudio.h`) and track first-highlight latency over time.
