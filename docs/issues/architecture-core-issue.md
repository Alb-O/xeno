# Architecture core issue: current state summary

Date: 2026-02-13

## Purpose

This document tracks the architecture split between `xeno-editor` (core engine) and `xeno-editor-tui` (frontend/runtime), with a concise status view instead of per-commit logs.

## Target model

- `xeno-editor` owns editor state, document operations, policies, effects, and backend-neutral render data.
- `xeno-editor-tui` owns terminal runtime, frame composition, widget rendering, and frontend interaction wiring.
- Engine/frontend coordination happens through data APIs and explicit seams, not internal field reach-through.

## Current state

### Ownership

- Frontend runtime/composition is in `xeno-editor-tui`.
- Core now owns behavior/data planning for completion, snippet choice, modal overlay panes, utility which-key, and statusline composition.
- Frontend rendering remains toolkit-specific (`xeno-editor-tui` owns widget composition, style mapping, and frame layering).

### Boundary hardening completed

- `xeno-editor` no longer has direct `xeno_tui` usage in `crates/editor/src` (current count: `0`).
- `xeno-editor` no longer publicly exposes broad internal modules:
  - `render` is internal; frontend imports via `render_api`.
  - `impls` is internal; focused symbols are re-exported at crate root where needed.
  - `notifications` internals are hidden behind typed editor APIs.
- Core render text primitives (`RenderLine`/`RenderSpan`) are backend-neutral and exposed through `render_api` for explicit frontend adaptation.
- Frontend no longer reaches through overlay internals for policy:
  - no `overlay_interaction()` usage in frontend crates.
  - no frontend overlay store reads for completion/snippet/status/utility behavior.
  - frontends consume typed plans/APIs (`overlay_pane_render_plan`, `whichkey_render_plan`, `statusline_render_plan`, `completion_render_plan`, `snippet_choice_render_plan`).
- `xeno-editor-tui` now consumes lifecycle hook and notification render surfaces through `xeno-editor` APIs and has no direct `xeno_registry` imports.
- `xeno-term` startup config loading/application also routes through `xeno-editor`, and `xeno-term` no longer imports `xeno_registry` directly.
- Legacy compatibility paths/shims removed:
  - focus compatibility helper path removed in favor of unified `set_focus` flow.
  - unused overlay compatibility constructor argument removed.
  - stale render seam exports and dead helper surface removed.
- Umbrella passthrough patterns were reduced:
  - callsites now import types from owning modules (`types`, `command_queue`, etc.).
- Overlay store accessors are now crate-private to prevent new frontend reach-through.
- Plan-builder regression tests now cover completion, snippet-choice, and statusline policy outputs.

### Feature posture

- `xeno-editor` supports headless builds (`--no-default-features`).
- `tui` remains an optional feature on `xeno-editor` (default-enabled), with frontend runtime in `xeno-editor-tui`.
- `xeno-editor-iced` exists as an experimental frontend crate behind `iced-wgpu` feature for GUI runtime probing without changing core ownership boundaries.

## Remaining work

- Decide whether panel registration/state should remain in core `UiManager` or move fully to frontend ownership.
- Finish backend-neutral text/style seam tightening (`Style`/`Line`/`Span` render boundary) for future GUI adapters.
- Port the experimental iced frontend from text snapshot rendering to full plan/render adapters shared with TUI policy outputs.
- Add replay/snapshot coverage for plan builders to guard cross-frontend behavior consistency.
- Keep pruning legacy/compat patterns as they appear during refactors.

## Acceptance checks

Use this matrix for boundary-sensitive changes:

```bash
cargo check -p xeno-editor --all-targets
cargo check -p xeno-editor --no-default-features --all-targets
cargo check -p xeno-editor-tui
cargo check -p xeno-term
```

Run targeted tests for touched areas (`focus`, `mouse`, `row`, panel/overlay tests) before merge.

## History policy

Detailed checkpoint history is intentionally kept in Git commit history and PRs.

Useful resume commands:

```bash
git log --oneline -n 40
rg -n "xeno_tui::|use xeno_tui" crates/editor/src -S
rg -n "xeno_editor::render::|xeno_editor::impls::|xeno_editor::notifications::" crates/editor-tui/src crates/term/src -S
```
