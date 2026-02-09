# Viewport-First Highlighting for L-tier Files

Status: **blocked** — partial parse produces incorrect highlights for files with multi-line constructs (block comments) spanning beyond the truncation point.

## Problem

Opening `tmp/miniaudio.h` (4.1MB, 96K lines, L-tier) takes ~885ms for a full tree-sitter parse. During this window the viewport shows un-highlighted text. The goal: parse only viewport bytes first (~2–5ms) for immediate highlighting, then complete the full parse in background.

## Architecture (was working, but not enough)

The scheduling infrastructure was fully implemented and functional:

- `TaskKind::ViewportParse` — new variant alongside `FullParse` / `Incremental`
- `SyntaxSlot.partial: bool` — tracks viewport-only trees; kept `dirty = true` so the next `ensure_syntax` schedules a full parse
- `EnsureSyntaxContext.viewport_end_byte: Option<u32>` — propagated from render path
- `SyntaxEngine::parse_viewport` trait method with default fallback to `parse`
- Scheduling gate: L-tier, no tree installed yet, `Visible` hotness, `viewport_end_byte > 0`
- Range: `0..min(viewport_end_byte * 2, file_size)` — 2x padding reduces re-parse on small scrolls
- `CancelFlag` — cooperative cancellation wired through tree-sitter's progress callback for all parse variants
- `ensure_locals` — viewport-bounded locals with geometric growth, decoupled from parse
- Debounce bypass for partial trees: `force_no_debounce = true` after viewport tree install, so the full parse follows immediately without the 250ms L-tier debounce gate

All of this compiled, passed tests, and scheduled correctly.

## What fails: the parse itself

### Approach 1: `set_included_ranges`

tree-sitter's `set_included_ranges` restricts which bytes the parser processes while preserving document-relative byte offsets. A `new_with_ranges_cancellable` constructor was added to tree-house to accept custom root layer ranges (reverted).

The problem: tree-sitter treats range boundaries as hard cuts. For `miniaudio.h`, the file opens with:

```c
/* ... short header comment ... */

/*
1. Introduction
===============
...  3724 lines of block comment ...
*/
```

The `/*` at line 12 has no matching `*/` within the viewport range (~100 lines). tree-sitter's error recovery kicks in: it sees `/*` with no close delimiter within the included range, produces an ERROR node, then re-enters normal parsing. Text inside the comment gets parsed as code, producing **wrong** highlights (not just missing highlights).

### Approach 2: source truncation

Instead of `set_included_ranges`, truncate the source rope to the viewport range end (snapped to a line boundary) and parse that prefix as a complete file. The theory: an unterminated `/*` at EOF should cause tree-sitter to extend the comment node to cover everything.

Same failure. tree-sitter's C grammar requires `*/` to close a `/*` block comment. An unterminated `/*` produces an ERROR node, not a comment spanning to EOF. The grammar rule is:

```
comment: $ => token(choice(
  seq('//', /.*/),
  seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')
))
```

The `/*..*/` pattern is a single regex token — there's no partial match. Without `*/`, the token fails and tree-sitter falls back to error recovery, same result as approach 1.

## Why this is fundamental

The issue isn't specific to C block comments. Any grammar with multi-line constructs that require closing delimiters will break:

- Block comments: `/* ... */` (C/C++/Java/Go/Rust/etc.)
- String literals: `""" ... """` (Python), `` ` ... ` `` (JS template literals), `r#" ... "#` (Rust raw strings)
- Heredocs: `<<EOF ... EOF` (shell/Ruby/Perl)

The grammar's tokenizer expects the closing delimiter within the parsed bytes. Without it, the token fails and error recovery produces incorrect AST structure.

## Potential directions (not attempted)

1. **Post-parse heuristic repair**: After the viewport parse, walk the tree looking for ERROR nodes at the truncation boundary. If the last token before truncation is inside `/*`, synthesize a comment span covering the remaining viewport bytes. This is grammar-specific and fragile but might work for the common case of block comments at file top.

2. **Injecting synthetic closing delimiters**: Before parsing the truncated source, scan for unclosed `/*` and append `*/` at the truncation point. This is the "bandaid fix" approach — it works for comments but not for all multi-line constructs, and the heuristic for detecting "unclosed" constructs is grammar-dependent.

3. **Two-pass with chunk boundaries**: Parse the first N bytes, then if the tree ends with ERROR, extend by another chunk and re-parse. Iterate until the ERROR resolves or a budget is exhausted. This converges but the worst case is parsing the whole file.

4. **Background-only with progressive render**: Don't parse a prefix at all. Instead, run the full parse on a background thread and progressively render highlights as tree-sitter's progress callback reports parsed byte ranges. Requires tree-sitter API changes to expose partial results during parsing.

5. **Accept wrong highlights for the flash**: The viewport-first tree was visible for ~885ms until the full parse completed. If the wrong highlights are considered acceptable as a transient state (better than no highlights), the implementation worked as-is. The question is whether wrong highlights are worse than no highlights from the user's perspective.

